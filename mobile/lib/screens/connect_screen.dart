import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:provider/provider.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../services/mcp_service.dart';

class ConnectScreen extends StatefulWidget {
  const ConnectScreen({super.key});
  @override
  State<ConnectScreen> createState() => _ConnectScreenState();
}

class _ConnectScreenState extends State<ConnectScreen> {
  final _hostCtrl = TextEditingController();
  final _portCtrl = TextEditingController(text: '7890');
  final _keyCtrl = TextEditingController();
  bool _connecting = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    _loadPrefs();
  }

  @override
  void dispose() {
    _hostCtrl.dispose();
    _portCtrl.dispose();
    _keyCtrl.dispose();
    super.dispose();
  }

  Future<void> _loadPrefs() async {
    final prefs = await SharedPreferences.getInstance();
    if (mounted) {
      setState(() {
        _hostCtrl.text = prefs.getString('host') ?? '';
        _portCtrl.text = (prefs.getInt('port') ?? 7890).toString();
        _keyCtrl.text = prefs.getString('apiKey') ?? '';
      });
    }
  }

  Future<void> _connect() async {
    HapticFeedback.lightImpact();
    final host = _hostCtrl.text.trim();
    final port = int.tryParse(_portCtrl.text.trim()) ?? 7890;
    final key = _keyCtrl.text.trim();

    if (host.isEmpty) {
      setState(() => _error = 'Enter your PC’s Tailscale IP (e.g. 100.x.y.z)');
      return;
    }

    final prefs = await SharedPreferences.getInstance();
    await prefs.setString('host', host);
    await prefs.setInt('port', port);
    await prefs.setString('apiKey', key);

    if (!mounted) return;
    final mcp = context.read<McpService>();
    mcp.host = host;
    mcp.port = port;
    mcp.apiKey = key.isEmpty ? null : key;

    setState(() { _connecting = true; _error = null; });
    try {
      await mcp.connect();
    } catch (e) {
      HapticFeedback.selectionClick();
      if (mounted) {
        setState(() { _error = e.toString().replaceFirst('Exception: ', ''); });
      }
    } finally {
      if (mounted) setState(() => _connecting = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: SafeArea(
        child: Center(
          child: SingleChildScrollView(
            padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 32),
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 420),
              child: Column(
                mainAxisAlignment: MainAxisAlignment.center,
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Center(
                    child: Container(
                      padding: const EdgeInsets.all(12),
                      decoration: BoxDecoration(
                        gradient: const LinearGradient(
                          colors: [Color(0xFF1E2640), Color(0xFF141622)],
                          begin: Alignment.topLeft,
                          end: Alignment.bottomRight,
                        ),
                        shape: BoxShape.circle,
                        border: Border.all(color: const Color(0xFF3B82F6).withValues(alpha: 0.3)),
                        boxShadow: [
                          BoxShadow(
                            color: const Color(0xFF3B82F6).withValues(alpha: 0.2),
                            blurRadius: 20,
                            spreadRadius: 2,
                          ),
                        ],
                      ),
                      child: Image.asset(
                        'assets/icon.png',
                        width: 56,
                        height: 56,
                        fit: BoxFit.contain,
                        errorBuilder: (_, __, ___) => const Icon(
                          Icons.timer_outlined,
                          size: 48,
                          color: Color(0xFF3B82F6),
                        ),
                      ),
                    ),
                  ),

                  const SizedBox(height: 20),

                  const Text(
                    'AutoClick Remote',
                    textAlign: TextAlign.center,
                    style: TextStyle(
                      fontSize: 22,
                      fontWeight: FontWeight.w800,
                      letterSpacing: -0.5,
                      color: Color(0xFFF1F5F9),
                    ),
                  ),

                  const SizedBox(height: 4),

                  const Text(
                    'Control Windows automation safely over Tailscale',
                    textAlign: TextAlign.center,
                    style: TextStyle(
                      fontSize: 13,
                      color: Color(0xFF8B92A5),
                    ),
                  ),

                  const SizedBox(height: 28),

                  // Form card
                  Container(
                    padding: const EdgeInsets.all(20),
                    decoration: BoxDecoration(
                      color: const Color(0xFF14151E),
                      borderRadius: BorderRadius.circular(16),
                      border: Border.all(color: const Color(0xFF242838)),
                    ),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        const Row(
                          children: [
                            Icon(Icons.hub_outlined, size: 16, color: Color(0xFF38BDF8)),
                            SizedBox(width: 8),
                            Text(
                              'SERVER CONFIGURATION',
                              style: TextStyle(
                                fontSize: 11,
                                fontWeight: FontWeight.w700,
                                letterSpacing: 0.8,
                                color: Color(0xFF6B7280),
                              ),
                            ),
                          ],
                        ),
                        const SizedBox(height: 14),
                        TextField(
                          controller: _hostCtrl,
                          style: const TextStyle(fontSize: 14, fontFamily: 'monospace'),
                          decoration: const InputDecoration(
                            labelText: 'Tailscale IP / Hostname',
                            hintText: '100.x.y.z',
                            prefixIcon: Icon(Icons.lan_outlined, size: 18, color: Color(0xFF8B92A5)),
                          ),
                          keyboardType: TextInputType.url,
                        ),
                        const SizedBox(height: 12),
                        Row(
                          children: [
                            Expanded(
                              flex: 2,
                              child: TextField(
                                controller: _portCtrl,
                                style: const TextStyle(fontSize: 14, fontFamily: 'monospace'),
                                decoration: const InputDecoration(
                                  labelText: 'Port',
                                ),
                                keyboardType: TextInputType.number,
                              ),
                            ),
                            const SizedBox(width: 10),
                            Expanded(
                              flex: 3,
                              child: TextField(
                                controller: _keyCtrl,
                                style: const TextStyle(fontSize: 14),
                                decoration: const InputDecoration(
                                  labelText: 'API Key (Optional)',
                                ),
                                obscureText: true,
                              ),
                            ),
                          ],
                        ),
                        if (_error != null) ...[
                          const SizedBox(height: 14),
                          Container(
                            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
                            decoration: BoxDecoration(
                              color: const Color(0xFFEF4444).withValues(alpha: 0.1),
                              borderRadius: BorderRadius.circular(8),
                              border: Border.all(color: const Color(0xFFEF4444).withValues(alpha: 0.3)),
                            ),
                            child: Row(
                              children: [
                                const Icon(Icons.info_outline, color: Color(0xFFEF4444), size: 16),
                                const SizedBox(width: 8),
                                Expanded(
                                  child: Text(
                                    _error!,
                                    style: const TextStyle(
                                      color: Color(0xFFEF4444),
                                      fontSize: 12,
                                      fontWeight: FontWeight.w500,
                                    ),
                                  ),
                                ),
                              ],
                            ),
                          ),
                        ],
                        const SizedBox(height: 20),
                        ElevatedButton(
                          onPressed: _connecting ? null : _connect,
                          style: ElevatedButton.styleFrom(
                            backgroundColor: const Color(0xFF3B82F6),
                            foregroundColor: Colors.white,
                            padding: const EdgeInsets.symmetric(vertical: 14),
                          ),
                          child: _connecting
                              ? const SizedBox(
                                  width: 18,
                                  height: 18,
                                  child: CircularProgressIndicator(
                                    strokeWidth: 2,
                                    color: Colors.white,
                                  ),
                                )
                              : const Text('Connect to Host PC'),
                        ),
                      ],
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}
