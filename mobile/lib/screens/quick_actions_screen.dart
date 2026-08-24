import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:provider/provider.dart';

import '../services/mcp_service.dart';
import '../widgets/action_tile.dart';
import '../widgets/duration_picker.dart';
import '../widgets/sleep_wake_modal.dart';
import '../widgets/lockout_safeguard_dialog.dart';

class QuickActionsScreen extends StatefulWidget {
  const QuickActionsScreen({super.key});
  @override
  State<QuickActionsScreen> createState() => _QuickActionsScreenState();
}

class _QuickActionsScreenState extends State<QuickActionsScreen> {
  int _afterSeconds = 5;
  bool _caffeineActive = false;
  bool _loading = false;
  final _promptCtrl = TextEditingController();

  @override
  void initState() {
    super.initState();
    _fetchCaffeineStatus();
  }

  Future<void> _fetchCaffeineStatus() async {
    final mcp = context.read<McpService>();
    if (!mcp.isConnected) return;
    try {
      final s = await mcp.getStatus();
      if (mounted && s.containsKey('caffeine_active')) {
        setState(() {
          _caffeineActive = s['caffeine_active'] == true;
        });
      }
    } catch (_) {}
  }

  @override
  void dispose() {
    _promptCtrl.dispose();
    super.dispose();
  }

  Future<void> _fire(String action, [Map<String, dynamic>? extra, String? customSuccessMsg]) async {
    HapticFeedback.lightImpact();
    final mcp = context.read<McpService>();
    setState(() => _loading = true);
    try {
      final args = {
        'action': action,
        'after': _afterSeconds,
        if (extra != null) ...extra,
      };
      await mcp.executeAction(args);
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Row(
              children: [
                const Icon(Icons.check_circle, color: Colors.white, size: 18),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    customSuccessMsg ?? '${action.toUpperCase()} scheduled in ${_fmt(_afterSeconds)}',
                    style: const TextStyle(fontWeight: FontWeight.w600),
                  ),
                ),
              ],
            ),
            backgroundColor: const Color(0xFF10B981),
            behavior: SnackBarBehavior.floating,
            shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
            duration: const Duration(seconds: 3),
          ),
        );
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Error: $e'),
            backgroundColor: const Color(0xFFEF4444),
            behavior: SnackBarBehavior.floating,
            shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
          ),
        );
      }
    } finally {
      if (mounted) setState(() => _loading = false);
    }
  }

  Future<void> _triggerSleep() async {
    HapticFeedback.lightImpact();
    final result = await SleepWakeModal.show(
      context,
      initialDurationSeconds: 3600, // 1 hour default
      initialGraceSeconds: 5,
    );

    if (result != null) {
      await _fire(
        'sleep',
        {
          'after': result.sleepDurationSeconds,
          'pre_sleep_grace': result.preSleepGraceSeconds,
        },
        'Sleep scheduled (${_fmtDuration(result.sleepDurationSeconds)}) • RTC wake armed',
      );
    }
  }

  Future<void> _triggerShutdown() async {
    HapticFeedback.mediumImpact();
    final delay = await LockoutSafeguardDialog.show(context, initialDelaySeconds: 10);
    if (delay != null) {
      await _fire(
        'shutdown',
        {'after': delay},
        'Shutting down PC in ${delay}s',
      );
    }
  }

  Future<void> _toggleCaffeine() async {
    HapticFeedback.selectionClick();
    final mcp = context.read<McpService>();
    setState(() => _loading = true);
    try {
      await mcp.setCaffeine(!_caffeineActive);
      setState(() => _caffeineActive = !_caffeineActive);
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Error: $e'), backgroundColor: const Color(0xFFEF4444)),
        );
      }
    } finally {
      if (mounted) setState(() => _loading = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return SingleChildScrollView(
      padding: const EdgeInsets.all(16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // Step Delay header
          const Text(
            'INPUT ACTION DELAY',
            style: TextStyle(
              fontSize: 11,
              fontWeight: FontWeight.w700,
              letterSpacing: 0.8,
              color: Color(0xFF6B7280),
            ),
          ),
          const SizedBox(height: 8),
          DurationPicker(
            value: _afterSeconds,
            onChanged: (v) => setState(() => _afterSeconds = v),
          ),

          const SizedBox(height: 20),

          // Input Automation Group
          const Text(
            'INPUT AUTOMATION',
            style: TextStyle(
              fontSize: 11,
              fontWeight: FontWeight.w700,
              letterSpacing: 0.8,
              color: Color(0xFF6B7280),
            ),
          ),
          const SizedBox(height: 8),

          ActionTile(
            title: 'Enter Key',
            subtitle: 'Injects Return keystroke after ${_fmt(_afterSeconds)}',
            icon: Icons.keyboard_return,
            accentColor: const Color(0xFF38BDF8),
            badgeText: 'INSTANT',
            disabled: _loading,
            onTap: () => _fire('enter'),
          ),
          ActionTile(
            title: 'Mouse Click',
            subtitle: 'Simulates left mouse button click at cursor position',
            icon: Icons.mouse,
            accentColor: const Color(0xFF3B82F6),
            badgeText: 'MOUSE',
            disabled: _loading,
            onTap: () => _fire('click'),
          ),

          const SizedBox(height: 4),

          // Clean Type text input card
          Container(
            padding: const EdgeInsets.all(16),
            decoration: BoxDecoration(
              color: const Color(0xFF14151E),
              borderRadius: BorderRadius.circular(12),
              border: Border.all(color: const Color(0xFF242838)),
            ),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                const Row(
                  children: [
                    Icon(Icons.keyboard_outlined, size: 18, color: Color(0xFF8B92A5)),
                    SizedBox(width: 8),
                    Text('Type String Keystrokes', style: TextStyle(fontWeight: FontWeight.w600, fontSize: 13.5)),
                  ],
                ),
                const SizedBox(height: 12),
                TextField(
                  controller: _promptCtrl,
                  style: const TextStyle(fontSize: 14),
                  decoration: const InputDecoration(
                    hintText: 'Enter text to type remotely...',
                    contentPadding: EdgeInsets.symmetric(horizontal: 14, vertical: 12),
                  ),
                ),
                const SizedBox(height: 10),
                ElevatedButton.icon(
                  onPressed: _loading
                      ? null
                      : () {
                          if (_promptCtrl.text.isNotEmpty) {
                            _fire('type', {'prompt': _promptCtrl.text});
                          }
                        },
                  icon: const Icon(Icons.send_rounded, size: 16),
                  label: Text('Send Keystrokes (${_fmt(_afterSeconds)} delay)'),
                  style: ElevatedButton.styleFrom(
                    backgroundColor: const Color(0xFF3B82F6),
                    padding: const EdgeInsets.symmetric(vertical: 12),
                  ),
                ),
              ],
            ),
          ),

          const SizedBox(height: 24),

          // Power & Remote Management Group
          Row(
            children: [
              const Text(
                'POWER & REMOTE SAFEGUARDS',
                style: TextStyle(
                  fontSize: 11,
                  fontWeight: FontWeight.w700,
                  letterSpacing: 0.8,
                  color: Color(0xFF6B7280),
                ),
              ),
              const Spacer(),
              Container(
                padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                decoration: BoxDecoration(
                  color: const Color(0xFF38BDF8).withValues(alpha: 0.12),
                  borderRadius: BorderRadius.circular(4),
                ),
                child: const Row(
                  children: [
                    Icon(Icons.lock_outline, size: 10, color: Color(0xFF38BDF8)),
                    SizedBox(width: 3),
                    Text('LOCKOUT GUARD', style: TextStyle(fontSize: 9, fontWeight: FontWeight.bold, color: Color(0xFF38BDF8))),
                  ],
                ),
              ),
            ],
          ),
          const SizedBox(height: 8),

          ActionTile(
            title: 'Sleep PC (RTC Wake)',
            subtitle: 'Choose sleep duration & arm hardware alarm to wake automatically',
            icon: Icons.bedtime,
            accentColor: const Color(0xFFA855F7),
            badgeText: 'RTC ALARM',
            disabled: _loading,
            onTap: _triggerSleep,
          ),

          ActionTile(
            title: 'Shut Down PC',
            subtitle: 'Powers off host system completely (Lockout Warning Protected)',
            icon: Icons.power_settings_new,
            accentColor: const Color(0xFFEF4444),
            badgeText: 'SAFEGUARDED',
            disabled: _loading,
            onTap: _triggerShutdown,
          ),

          const SizedBox(height: 4),

          // Caffeine Card
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
            decoration: BoxDecoration(
              color: const Color(0xFF14151E),
              borderRadius: BorderRadius.circular(12),
              border: Border.all(
                color: _caffeineActive
                    ? const Color(0xFFF59E0B).withValues(alpha: 0.5)
                    : const Color(0xFF242838),
              ),
            ),
            child: Row(
              children: [
                Container(
                  padding: const EdgeInsets.all(8),
                  decoration: BoxDecoration(
                    color: const Color(0xFFF59E0B).withValues(alpha: 0.15),
                    borderRadius: BorderRadius.circular(10),
                  ),
                  child: const Icon(Icons.coffee, size: 20, color: Color(0xFFF59E0B)),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      const Text('Caffeine Keep-Awake', style: TextStyle(fontWeight: FontWeight.w600, fontSize: 13.5)),
                      Text(
                        _caffeineActive ? 'Active: PC screen and system will not sleep' : 'Inactive: Normal Windows sleep schedule',
                        style: TextStyle(
                          color: _caffeineActive ? const Color(0xFFFBBF24) : const Color(0xFF8B92A5),
                          fontSize: 11.5,
                        ),
                      ),
                    ],
                  ),
                ),
                Switch(
                  value: _caffeineActive,
                  activeThumbColor: const Color(0xFFF59E0B),
                  activeTrackColor: const Color(0xFFF59E0B).withValues(alpha: 0.3),
                  onChanged: _loading ? null : (_) => _toggleCaffeine(),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  String _fmt(int s) => s >= 3600 ? '${s ~/ 3600}h ${(s % 3600) ~/ 60}m' : s >= 60 ? '${s ~/ 60}m ${s % 60}s' : '${s}s';

  String _fmtDuration(int s) {
    final h = s ~/ 3600;
    final m = (s % 3600) ~/ 60;
    if (h > 0 && m > 0) return '${h}h ${m}m';
    if (h > 0) return '${h}h';
    if (m > 0) return '${m}m';
    return '${s}s';
  }
}
