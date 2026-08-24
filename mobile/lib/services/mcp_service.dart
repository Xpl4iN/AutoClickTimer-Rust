import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'package:flutter/foundation.dart';

/// MCP TCP Client Service
///
/// Manages a persistent TCP connection to the AutoClickTimer MCP server
/// over Tailscale. Handles authentication, request/response matching,
/// and connection lifecycle.
class McpService extends ChangeNotifier {
  String host;
  int port;
  String? apiKey;

  Socket? _socket;
  StreamSubscription? _sub;
  final _pending = <dynamic, Completer<Map<String, dynamic>>>{};
  int _nextId = 1;
  bool _authenticated = false;

  McpConnectionState _state = McpConnectionState.disconnected;
  String? _lastError;

  McpConnectionState get state => _state;
  String? get lastError => _lastError;
  bool get isConnected => _state == McpConnectionState.connected && _authenticated;

  McpService({required this.host, required this.port, this.apiKey});

  Future<void> connect() async {
    if (_socket != null) await disconnect();
    _setState(McpConnectionState.connecting);
    _lastError = null;

    try {
      _socket = await Socket.connect(host, port, timeout: const Duration(seconds: 10));

      final lines = _socket!
          .cast<List<int>>()
          .transform(utf8.decoder)
          .transform(const LineSplitter());

      _sub = lines.listen(
        _onLine,
        onError: (e) {
          _lastError = e.toString();
          _setState(McpConnectionState.disconnected);
          _rejectAll('Connection error: $e');
        },
        onDone: () {
          _setState(McpConnectionState.disconnected);
          _rejectAll('Server disconnected');
        },
      );

      // MCP initialize handshake
      await _call('initialize', {
        'protocolVersion': '2024-11-05',
        'capabilities': {},
        'clientInfo': {'name': 'AutoClickTimer-Remote', 'version': '1.0.0'},
      });

      // API key auth if required
      if (apiKey != null && apiKey!.isNotEmpty) {
        final resp = await _call('auth', {'key': apiKey!});
        final authed = resp['result']?['authenticated'] as bool? ?? false;
        if (!authed) {
          _lastError = 'Authentication failed: wrong API key';
          await disconnect();
          throw Exception(_lastError);
        }
      }

      _authenticated = true;
      _setState(McpConnectionState.connected);
    } catch (e) {
      _lastError = e.toString();
      _setState(McpConnectionState.disconnected);
      rethrow;
    }
  }

  Future<void> disconnect() async {
    _authenticated = false;
    _sub?.cancel();
    _sub = null;
    await _socket?.close();
    _socket = null;
    _setState(McpConnectionState.disconnected);
    _rejectAll('Disconnected');
  }

  void _setState(McpConnectionState s) {
    _state = s;
    notifyListeners();
  }

  void _onLine(String line) {
    line = line.trim();
    if (line.isEmpty) return;
    try {
      final Map<String, dynamic> msg = jsonDecode(line);
      final id = msg['id'];
      if (id != null && _pending.containsKey(id)) {
        _pending.remove(id)!.complete(msg);
      }
    } catch (_) {}
  }

  void _rejectAll(String reason) {
    for (final c in _pending.values) {
      if (!c.isCompleted) c.completeError(Exception(reason));
    }
    _pending.clear();
  }

  Future<void> _send(Map<String, dynamic> msg) async {
    _socket?.write('${jsonEncode(msg)}\n');
  }

  Future<Map<String, dynamic>> _call(String method, [Map<String, dynamic>? params]) async {
    final id = _nextId++;
    final completer = Completer<Map<String, dynamic>>();
    _pending[id] = completer;
    await _send({'jsonrpc': '2.0', 'id': id, 'method': method, 'params': params ?? {}});
    return completer.future.timeout(const Duration(seconds: 30), onTimeout: () {
      _pending.remove(id);
      throw TimeoutException('Request "$method" timed out', const Duration(seconds: 30));
    });
  }

  // -------------------------------------------------------------------------
  // Public MCP tool wrappers
  // -------------------------------------------------------------------------

  Future<Map<String, dynamic>> callTool(String name, [Map<String, dynamic>? args]) async {
    final resp = await _call('tools/call', {'name': name, 'arguments': args ?? {}});
    if (resp['error'] != null) throw Exception(resp['error']['message']);
    final content = (resp['result']?['content'] as List?)?.first;
    final text = content?['text'] as String? ?? '';
    try {
      return jsonDecode(text) as Map<String, dynamic>;
    } catch (_) {
      return {'raw': text};
    }
  }

  Future<Map<String, dynamic>> getStatus() => callTool('act_get_status');
  Future<Map<String, dynamic>> cancel() => callTool('act_cancel');
  Future<Map<String, dynamic>> getCursorPos() => callTool('act_get_cursor_pos');

  Future<List<String>> listWindows() async {
    final r = await callTool('act_list_windows');
    return (r['windows'] as List? ?? []).map((e) => e.toString()).toList();
  }

  Future<Map<String, dynamic>> setCaffeine(bool active, [int? durationSeconds]) =>
      callTool('act_set_caffeine', {
        'active': active,
        if (durationSeconds != null) 'duration_seconds': durationSeconds,
      });

  Future<Map<String, dynamic>> executeAction(Map<String, dynamic> args) =>
      callTool('act_execute_action', args);

  Future<Map<String, dynamic>> scheduleQueue(
      List<Map<String, dynamic>> steps, [int repeat = 1]) =>
      callTool('act_schedule_queue', {'steps': steps, 'repeat_count': repeat});

  Future<Map<String, dynamic>> runProfile(String path, [int repeat = 1]) =>
      callTool('act_run_profile', {'profile_path': path, 'repeat_count': repeat});

  Future<Map<String, dynamic>> saveProfile(String path, List<Map<String, dynamic>> steps) =>
      callTool('act_save_profile', {'profile_path': path, 'steps': steps});

  Future<Map<String, dynamic>> reorderQueue(int from, int to, [String? profilePath]) =>
      callTool('act_reorder_queue', {
        'from_index': from,
        'to_index': to,
        if (profilePath != null) 'profile_path': profilePath,
      });

  Future<Map<String, dynamic>> configurePasswordlessWake() =>
      callTool('act_configure_passwordless_wake');
}

enum McpConnectionState { disconnected, connecting, connected }
