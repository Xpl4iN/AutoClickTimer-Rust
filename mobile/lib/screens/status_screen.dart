import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:provider/provider.dart';

import '../services/mcp_service.dart';
import '../widgets/stat_card.dart';
import '../widgets/action_chip_row.dart';
import '../widgets/sleep_wake_modal.dart';
import '../widgets/lockout_safeguard_dialog.dart';

class StatusScreen extends StatefulWidget {
  const StatusScreen({super.key});
  @override
  State<StatusScreen> createState() => _StatusScreenState();
}

class _StatusScreenState extends State<StatusScreen> {
  Map<String, dynamic>? _status;
  Timer? _timer;
  bool _loading = true;
  String? _error;

  @override
  void initState() {
    super.initState();
    _refresh();
    _timer = Timer.periodic(const Duration(seconds: 1), (_) => _refresh());
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  Future<void> _refresh() async {
    final mcp = context.read<McpService>();
    if (!mcp.isConnected) return;
    try {
      final s = await mcp.getStatus();
      if (mounted) setState(() { _status = s; _loading = false; _error = null; });
    } catch (e) {
      if (mounted) setState(() { _error = e.toString(); _loading = false; });
    }
  }

  Future<void> _cancel() async {
    HapticFeedback.mediumImpact();
    final mcp = context.read<McpService>();
    try {
      await mcp.cancel();
      await _refresh();
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: const Row(
              children: [
                Icon(Icons.stop_circle_outlined, color: Colors.white, size: 18),
                SizedBox(width: 8),
                Text('Queue / Timer Stopped', style: TextStyle(fontWeight: FontWeight.w600)),
              ],
            ),
            backgroundColor: const Color(0xFF10B981),
            behavior: SnackBarBehavior.floating,
            shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
            duration: const Duration(seconds: 2),
          ),
        );
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Error: $e'), backgroundColor: const Color(0xFFEF4444)),
        );
      }
    }
  }

  Future<void> _handleQuickAction(String action) async {
    HapticFeedback.lightImpact();
    final mcp = context.read<McpService>();

    if (action == 'sleep') {
      final result = await SleepWakeModal.show(context, initialDurationSeconds: 3600, initialGraceSeconds: 5);
      if (result != null && mounted) {
        try {
          await mcp.executeAction({
            'action': 'sleep',
            'after': result.sleepDurationSeconds,
            'pre_sleep_grace': result.preSleepGraceSeconds,
          });
          if (mounted) {
            ScaffoldMessenger.of(context).showSnackBar(
              SnackBar(
                content: Text('Sleep scheduled (${_fmtDuration(result.sleepDurationSeconds)}) • RTC wake armed'),
                backgroundColor: const Color(0xFF10B981),
                behavior: SnackBarBehavior.floating,
                shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
              ),
            );
          }
        } catch (e) {
          if (mounted) {
            ScaffoldMessenger.of(context).showSnackBar(
              SnackBar(content: Text('Error: $e'), backgroundColor: const Color(0xFFEF4444)),
            );
          }
        }
      }
      return;
    }

    if (action == 'shutdown') {
      final delay = await LockoutSafeguardDialog.show(context, initialDelaySeconds: 10);
      if (delay != null && mounted) {
        try {
          await mcp.executeAction({'action': 'shutdown', 'after': delay});
          if (mounted) {
            ScaffoldMessenger.of(context).showSnackBar(
              SnackBar(
                content: Text('Shutdown scheduled in ${delay}s'),
                backgroundColor: const Color(0xFFEF4444),
                behavior: SnackBarBehavior.floating,
                shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
              ),
            );
          }
        } catch (e) {
          if (mounted) {
            ScaffoldMessenger.of(context).showSnackBar(
              SnackBar(content: Text('Error: $e'), backgroundColor: const Color(0xFFEF4444)),
            );
          }
        }
      }
      return;
    }

    // Default fast actions (click, enter)
    try {
      await mcp.executeAction({'action': action, 'after': 5});
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('${action.toUpperCase()} armed (5s delay)'),
            backgroundColor: const Color(0xFF10B981),
            behavior: SnackBarBehavior.floating,
            shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
            duration: const Duration(seconds: 2),
          ),
        );
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Error: $e'), backgroundColor: const Color(0xFFEF4444)),
        );
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final s = _status;

    if (_loading && s == null) {
      return const Center(
        child: CircularProgressIndicator(color: Color(0xFF3B82F6)),
      );
    }

    if (_error != null && s == null) {
      return Center(
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Container(
                padding: const EdgeInsets.all(16),
                decoration: BoxDecoration(
                  color: const Color(0xFFEF4444).withValues(alpha: 0.1),
                  shape: BoxShape.circle,
                ),
                child: const Icon(Icons.cloud_off_outlined, color: Color(0xFFEF4444), size: 36),
              ),
              const SizedBox(height: 16),
              const Text('Connection lost', style: TextStyle(fontWeight: FontWeight.bold, fontSize: 16)),
              const SizedBox(height: 6),
              Text(_error!, style: const TextStyle(color: Color(0xFF8B92A5), fontSize: 13), textAlign: TextAlign.center),
              const SizedBox(height: 20),
              ElevatedButton(onPressed: _refresh, child: const Text('Retry Connection')),
            ],
          ),
        ),
      );
    }

    final isRunning = s?['is_running'] as bool? ?? false;
    final phase = s?['phase'] as String? ?? 'idle';
    final remaining = s?['remaining_seconds'] as int? ?? 0;
    final current = s?['current_step'] as String? ?? '-';
    final iteration = s?['current_iteration'] as int? ?? 1;
    final total = s?['total_steps'] as int? ?? 0;
    final isSleepPhase = phase.toLowerCase().contains('sleep') || phase.toLowerCase().contains('grace');

    return RefreshIndicator(
      onRefresh: _refresh,
      color: const Color(0xFF3B82F6),
      child: SingleChildScrollView(
        physics: const AlwaysScrollableScrollPhysics(),
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            // State Header Hero Card
            Container(
              padding: const EdgeInsets.all(20),
              decoration: BoxDecoration(
                gradient: LinearGradient(
                  colors: isRunning
                      ? isSleepPhase
                          ? [const Color(0xFF221A33), const Color(0xFF14151E)]
                          : [const Color(0xFF132338), const Color(0xFF14151E)]
                      : [const Color(0xFF161822), const Color(0xFF12131A)],
                  begin: Alignment.topLeft,
                  end: Alignment.bottomRight,
                ),
                borderRadius: BorderRadius.circular(16),
                border: Border.all(
                  color: isRunning
                      ? isSleepPhase
                          ? const Color(0xFFA855F7).withValues(alpha: 0.5)
                          : const Color(0xFF3B82F6).withValues(alpha: 0.5)
                      : const Color(0xFF242838),
                  width: 1.2,
                ),
              ),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Row(
                    mainAxisAlignment: MainAxisAlignment.spaceBetween,
                    children: [
                      Row(
                        children: [
                          Container(
                            width: 10,
                            height: 10,
                            decoration: BoxDecoration(
                              shape: BoxShape.circle,
                              color: isRunning ? const Color(0xFF10B981) : const Color(0xFF64748B),
                              boxShadow: isRunning
                                  ? [
                                      BoxShadow(
                                        color: const Color(0xFF10B981).withValues(alpha: 0.6),
                                        blurRadius: 8,
                                        spreadRadius: 2,
                                      )
                                    ]
                                  : null,
                            ),
                          ),
                          const SizedBox(width: 10),
                          Text(
                            isRunning ? 'ACTIVE EXECUTION' : 'HOST IDLE',
                            style: TextStyle(
                              fontSize: 12,
                              fontWeight: FontWeight.w800,
                              letterSpacing: 0.8,
                              color: isRunning ? const Color(0xFF10B981) : const Color(0xFF8B92A5),
                            ),
                          ),
                        ],
                      ),
                      Container(
                        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
                        decoration: BoxDecoration(
                          color: const Color(0xFF1E2232),
                          borderRadius: BorderRadius.circular(6),
                          border: Border.all(color: const Color(0xFF2E344A)),
                        ),
                        child: Text(
                          phase.toUpperCase(),
                          style: const TextStyle(
                            fontSize: 11,
                            fontWeight: FontWeight.w700,
                            color: Color(0xFF94A3B8),
                            fontFamily: 'monospace',
                          ),
                        ),
                      ),
                    ],
                  ),
                  const SizedBox(height: 18),
                  if (isRunning) ...[
                    Text(
                      _fmt(remaining),
                      style: const TextStyle(
                        fontSize: 42,
                        fontWeight: FontWeight.w800,
                        letterSpacing: -1.5,
                        fontFeatures: [FontFeature.tabularFigures()],
                        color: Color(0xFFF1F5F9),
                      ),
                    ),
                    const SizedBox(height: 4),
                    Row(
                      children: [
                        Icon(
                          isSleepPhase ? Icons.bedtime : Icons.timelapse,
                          size: 14,
                          color: const Color(0xFF8B92A5),
                        ),
                        const SizedBox(width: 6),
                        Text(
                          isSleepPhase
                              ? 'RTC Hardware wake active • Windows suspended'
                              : 'Time remaining in current queue step',
                          style: const TextStyle(fontSize: 12, color: Color(0xFF8B92A5)),
                        ),
                      ],
                    ),
                  ] else ...[
                    const Text(
                      'Ready for Command',
                      style: TextStyle(
                        fontSize: 24,
                        fontWeight: FontWeight.w700,
                        color: Color(0xFFF1F5F9),
                        letterSpacing: -0.4,
                      ),
                    ),
                    const SizedBox(height: 4),
                    const Text(
                      'No active queue or timer running on host PC.',
                      style: TextStyle(fontSize: 13, color: Color(0xFF8B92A5)),
                    ),
                  ],
                ],
              ),
            ),

            const SizedBox(height: 12),

            // Metadata Grid
            Row(
              children: [
                Expanded(
                  child: StatCard(
                    label: 'Iteration',
                    value: total > 0 ? '$iteration / $total' : '1 / 1',
                  ),
                ),
                const SizedBox(width: 10),
                Expanded(
                  child: StatCard(
                    label: 'Current Action',
                    value: current == '-' ? 'None' : current,
                  ),
                ),
              ],
            ),

            const SizedBox(height: 16),

            if (isRunning) ...[
              ElevatedButton.icon(
                onPressed: _cancel,
                icon: const Icon(Icons.stop, size: 20),
                label: const Text('Emergency Stop Queue'),
                style: ElevatedButton.styleFrom(
                  backgroundColor: const Color(0xFFEF4444),
                  foregroundColor: Colors.white,
                  padding: const EdgeInsets.symmetric(vertical: 14),
                  shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
                ),
              ),
            ] else ...[
              const Padding(
                padding: EdgeInsets.symmetric(vertical: 8),
                child: Row(
                  children: [
                    Text(
                      'QUICK DISPATCH (ARMED WITH SAFEGUARDS)',
                      style: TextStyle(
                        fontSize: 11,
                        fontWeight: FontWeight.w700,
                        letterSpacing: 0.8,
                        color: Color(0xFF6B7280),
                      ),
                    ),
                  ],
                ),
              ),
              ActionChipRow(
                onQuickAction: _handleQuickAction,
              ),
            ],
          ],
        ),
      ),
    );
  }

  String _fmt(int secs) {
    if (secs >= 3600) return '${secs ~/ 3600}h ${(secs % 3600) ~/ 60}m ${secs % 60}s';
    if (secs >= 60) return '${secs ~/ 60}m ${secs % 60}s';
    return '${secs}s';
  }

  String _fmtDuration(int s) {
    final h = s ~/ 3600;
    final m = (s % 3600) ~/ 60;
    if (h > 0 && m > 0) return '${h}h ${m}m';
    if (h > 0) return '${h}h';
    if (m > 0) return '${m}m';
    return '${s}s';
  }
}
