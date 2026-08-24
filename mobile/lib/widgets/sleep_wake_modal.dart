import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

class SleepConfigResult {
  final int sleepDurationSeconds;
  final int preSleepGraceSeconds;

  const SleepConfigResult({
    required this.sleepDurationSeconds,
    required this.preSleepGraceSeconds,
  });
}

class SleepWakeModal extends StatefulWidget {
  final int initialDurationSeconds;
  final int initialGraceSeconds;

  const SleepWakeModal({
    super.key,
    this.initialDurationSeconds = 3600, // Default: 1 hour
    this.initialGraceSeconds = 5,
  });

  static Future<SleepConfigResult?> show(
    BuildContext context, {
    int initialDurationSeconds = 3600,
    int initialGraceSeconds = 5,
  }) {
    return showModalBottomSheet<SleepConfigResult>(
      context: context,
      isScrollControlled: true,
      backgroundColor: const Color(0xFF12141C),
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(20)),
      ),
      builder: (_) => SleepWakeModal(
        initialDurationSeconds: initialDurationSeconds,
        initialGraceSeconds: initialGraceSeconds,
      ),
    );
  }

  @override
  State<SleepWakeModal> createState() => _SleepWakeModalState();
}

class _SleepWakeModalState extends State<SleepWakeModal> {
  late int _durationSeconds;
  late int _graceSeconds;
  bool _isCustom = false;
  late TextEditingController _hoursCtrl;
  late TextEditingController _minsCtrl;
  Timer? _clockTimer;
  DateTime _now = DateTime.now();

  static const _presets = [
    ('15m', 15 * 60),
    ('30m', 30 * 60),
    ('1h', 1 * 3600),
    ('2h', 2 * 3600),
    ('4h', 4 * 3600),
    ('8h', 8 * 3600),
  ];

  static const _gracePresets = [
    ('5s (Fast)', 5),
    ('10s', 10),
    ('30s (Safe)', 30),
    ('60s', 60),
  ];

  @override
  void initState() {
    super.initState();
    _durationSeconds = widget.initialDurationSeconds;
    _graceSeconds = widget.initialGraceSeconds;

    final hours = _durationSeconds ~/ 3600;
    final mins = (_durationSeconds % 3600) ~/ 60;
    _hoursCtrl = TextEditingController(text: hours > 0 ? hours.toString() : '0');
    _minsCtrl = TextEditingController(text: mins > 0 ? mins.toString() : '0');

    _isCustom = !_presets.any((p) => p.$2 == _durationSeconds);

    _clockTimer = Timer.periodic(const Duration(seconds: 1), (_) {
      if (mounted) setState(() => _now = DateTime.now());
    });
  }

  @override
  void dispose() {
    _clockTimer?.cancel();
    _hoursCtrl.dispose();
    _minsCtrl.dispose();
    super.dispose();
  }

  void _applyCustomInputs() {
    final h = int.tryParse(_hoursCtrl.text.trim()) ?? 0;
    final m = int.tryParse(_minsCtrl.text.trim()) ?? 0;
    final total = (h * 3600) + (m * 60);
    if (total > 0) {
      setState(() => _durationSeconds = total);
    }
  }

  DateTime get _calculatedWakeTime {
    return _now.add(Duration(seconds: _graceSeconds + _durationSeconds));
  }

  String _formatDateTime(DateTime dt) {
    final hour = dt.hour.toString().padLeft(2, '0');
    final min = dt.minute.toString().padLeft(2, '0');
    final sec = dt.second.toString().padLeft(2, '0');
    final isToday = dt.day == _now.day && dt.month == _now.month && dt.year == _now.year;
    
    final dayLabel = isToday ? 'Today' : 'Tomorrow (${dt.day}.${dt.month}.)';
    return '$dayLabel at $hour:$min:$sec';
  }

  String _formatDurationHuman(int secs) {
    final h = secs ~/ 3600;
    final m = (secs % 3600) ~/ 60;
    final s = secs % 60;
    if (h > 0 && m > 0) return '${h}h ${m}m';
    if (h > 0) return '${h}h';
    if (m > 0 && s > 0) return '${m}m ${s}s';
    if (m > 0) return '${m}m';
    return '${s}s';
  }

  @override
  Widget build(BuildContext context) {
    final bottomInset = MediaQuery.of(context).viewInsets.bottom;
    const border = Color(0xFF242838);

    return Padding(
      padding: EdgeInsets.fromLTRB(20, 16, 20, bottomInset + 20),
      child: SingleChildScrollView(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            // Drag handle
            Center(
              child: Container(
                width: 36,
                height: 4,
                decoration: BoxDecoration(
                  color: const Color(0xFF3B4254),
                  borderRadius: BorderRadius.circular(2),
                ),
              ),
            ),
            const SizedBox(height: 14),

            // Header
            Row(
              children: [
                Container(
                  padding: const EdgeInsets.all(8),
                  decoration: BoxDecoration(
                    color: const Color(0xFFA855F7).withValues(alpha: 0.15),
                    borderRadius: BorderRadius.circular(10),
                    border: Border.all(color: const Color(0xFFA855F7).withValues(alpha: 0.3)),
                  ),
                  child: const Icon(Icons.bedtime, color: Color(0xFFA855F7), size: 20),
                ),
                const SizedBox(width: 12),
                const Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        'Sleep PC (RTC Wake)',
                        style: TextStyle(
                          fontSize: 16,
                          fontWeight: FontWeight.w700,
                          color: Color(0xFFF1F5F9),
                        ),
                      ),
                      Text(
                        'Suspends PC and arms hardware RTC alarm to wake',
                        style: TextStyle(fontSize: 11, color: Color(0xFF8B92A5)),
                      ),
                    ],
                  ),
                ),
                IconButton(
                  icon: const Icon(Icons.close, size: 20, color: Color(0xFF8B92A5)),
                  onPressed: () => Navigator.pop(context),
                ),
              ],
            ),

            const SizedBox(height: 16),

            // Live Wake Preview Card
            Container(
              padding: const EdgeInsets.all(14),
              decoration: BoxDecoration(
                gradient: const LinearGradient(
                  colors: [Color(0xFF1E1A2E), Color(0xFF161824)],
                  begin: Alignment.topLeft,
                  end: Alignment.bottomRight,
                ),
                borderRadius: BorderRadius.circular(12),
                border: Border.all(color: const Color(0xFFA855F7).withValues(alpha: 0.4)),
              ),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Row(
                    children: [
                      const Icon(Icons.alarm, color: Color(0xFFA855F7), size: 16),
                      const SizedBox(width: 6),
                      const Text(
                        'AUTOMATIC HARDWARE WAKE',
                        style: TextStyle(
                          fontSize: 10,
                          fontWeight: FontWeight.w700,
                          letterSpacing: 0.6,
                          color: Color(0xFFA855F7),
                        ),
                      ),
                      const Spacer(),
                      Container(
                        padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                        decoration: BoxDecoration(
                          color: const Color(0xFF10B981).withValues(alpha: 0.2),
                          borderRadius: BorderRadius.circular(4),
                        ),
                        child: const Text(
                          'SAFE FOR REMOTE',
                          style: TextStyle(fontSize: 9, fontWeight: FontWeight.bold, color: Color(0xFF10B981)),
                        ),
                      ),
                    ],
                  ),
                  const SizedBox(height: 8),
                  Text(
                    _formatDateTime(_calculatedWakeTime),
                    style: const TextStyle(
                      fontSize: 18,
                      fontWeight: FontWeight.w700,
                      color: Color(0xFFF1F5F9),
                      fontFeatures: [FontFeature.tabularFigures()],
                    ),
                  ),
                  const SizedBox(height: 2),
                  Text(
                    'Sleep duration: ${_formatDurationHuman(_durationSeconds)} (plus ${_graceSeconds}s disconnect grace)',
                    style: const TextStyle(fontSize: 11, color: Color(0xFF94A3B8)),
                  ),
                ],
              ),
            ),

            const SizedBox(height: 16),

            // Sleep Duration Presets
            const Text(
              'HOW LONG TO SLEEP THE PC',
              style: TextStyle(
                fontSize: 11,
                fontWeight: FontWeight.w700,
                letterSpacing: 0.8,
                color: Color(0xFF6B7280),
              ),
            ),
            const SizedBox(height: 8),

            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                ..._presets.map((p) {
                  final selected = !_isCustom && _durationSeconds == p.$2;
                  return InkWell(
                    onTap: () {
                      HapticFeedback.selectionClick();
                      setState(() {
                        _durationSeconds = p.$2;
                        _isCustom = false;
                        _hoursCtrl.text = (p.$2 ~/ 3600).toString();
                        _minsCtrl.text = ((p.$2 % 3600) ~/ 60).toString();
                      });
                    },
                    borderRadius: BorderRadius.circular(8),
                    child: Container(
                      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 8),
                      decoration: BoxDecoration(
                        color: selected ? const Color(0xFFA855F7) : const Color(0xFF1A1D2B),
                        borderRadius: BorderRadius.circular(8),
                        border: Border.all(
                          color: selected ? const Color(0xFFA855F7) : border,
                        ),
                      ),
                      child: Text(
                        p.$1,
                        style: TextStyle(
                          fontSize: 13,
                          fontWeight: selected ? FontWeight.bold : FontWeight.w500,
                          color: selected ? Colors.white : const Color(0xFFCBD5E1),
                        ),
                      ),
                    ),
                  );
                }),
                InkWell(
                  onTap: () {
                    HapticFeedback.selectionClick();
                    setState(() => _isCustom = true);
                  },
                  borderRadius: BorderRadius.circular(8),
                  child: Container(
                    padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 8),
                    decoration: BoxDecoration(
                      color: _isCustom ? const Color(0xFFA855F7) : const Color(0xFF1A1D2B),
                      borderRadius: BorderRadius.circular(8),
                      border: Border.all(
                        color: _isCustom ? const Color(0xFFA855F7) : border,
                      ),
                    ),
                    child: Text(
                      'Custom...',
                      style: TextStyle(
                        fontSize: 13,
                        fontWeight: _isCustom ? FontWeight.bold : FontWeight.w500,
                        color: _isCustom ? Colors.white : const Color(0xFFCBD5E1),
                      ),
                    ),
                  ),
                ),
              ],
            ),

            if (_isCustom) ...[
              const SizedBox(height: 12),
              Row(
                children: [
                  Expanded(
                    child: TextField(
                      controller: _hoursCtrl,
                      keyboardType: TextInputType.number,
                      decoration: const InputDecoration(
                        labelText: 'Hours',
                        contentPadding: EdgeInsets.symmetric(horizontal: 12, vertical: 10),
                      ),
                      onChanged: (_) => _applyCustomInputs(),
                    ),
                  ),
                  const SizedBox(width: 10),
                  Expanded(
                    child: TextField(
                      controller: _minsCtrl,
                      keyboardType: TextInputType.number,
                      decoration: const InputDecoration(
                        labelText: 'Minutes',
                        contentPadding: EdgeInsets.symmetric(horizontal: 12, vertical: 10),
                      ),
                      onChanged: (_) => _applyCustomInputs(),
                    ),
                  ),
                ],
              ),
            ],

            const SizedBox(height: 16),

            // Pre-Sleep Grace Delay
            const Text(
              'DISCONNECT GRACE DELAY (BEFORE SUSPEND)',
              style: TextStyle(
                fontSize: 11,
                fontWeight: FontWeight.w700,
                letterSpacing: 0.8,
                color: Color(0xFF6B7280),
              ),
            ),
            const SizedBox(height: 8),

            Row(
              children: _gracePresets.map((g) {
                final selected = _graceSeconds == g.$2;
                return Expanded(
                  child: Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 3),
                    child: InkWell(
                      onTap: () {
                        HapticFeedback.selectionClick();
                        setState(() => _graceSeconds = g.$2);
                      },
                      borderRadius: BorderRadius.circular(8),
                      child: Container(
                        padding: const EdgeInsets.symmetric(vertical: 8),
                        decoration: BoxDecoration(
                          color: selected ? const Color(0xFF3B82F6) : const Color(0xFF1A1D2B),
                          borderRadius: BorderRadius.circular(8),
                          border: Border.all(
                            color: selected ? const Color(0xFF3B82F6) : border,
                          ),
                        ),
                        child: Center(
                          child: Text(
                            g.$1,
                            style: TextStyle(
                              fontSize: 11,
                              fontWeight: selected ? FontWeight.bold : FontWeight.w500,
                              color: selected ? Colors.white : const Color(0xFF8B92A5),
                            ),
                          ),
                        ),
                      ),
                    ),
                  ),
                );
              }).toList(),
            ),

            const SizedBox(height: 14),

            // Safety Hint
            Container(
              padding: const EdgeInsets.all(10),
              decoration: BoxDecoration(
                color: const Color(0xFF1E293B).withValues(alpha: 0.4),
                borderRadius: BorderRadius.circular(8),
                border: Border.all(color: const Color(0xFF334155)),
              ),
              child: const Row(
                children: [
                  Icon(Icons.shield_outlined, size: 16, color: Color(0xFF38BDF8)),
                  SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      'RTC timer wakes hardware even if Tailscale is idle. Zero-Password Wake ensures immediate reconnection.',
                      style: TextStyle(fontSize: 11, color: Color(0xFF94A3B8)),
                    ),
                  ),
                ],
              ),
            ),

            const SizedBox(height: 18),

            // Action Button
            ElevatedButton(
              onPressed: () {
                HapticFeedback.mediumImpact();
                Navigator.pop(
                  context,
                  SleepConfigResult(
                    sleepDurationSeconds: _durationSeconds,
                    preSleepGraceSeconds: _graceSeconds,
                  ),
                );
              },
              style: ElevatedButton.styleFrom(
                backgroundColor: const Color(0xFFA855F7),
                foregroundColor: Colors.white,
                padding: const EdgeInsets.symmetric(vertical: 14),
                shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
              ),
              child: Text(
                'Arm RTC Wake & Sleep (${_formatDurationHuman(_durationSeconds)})',
                style: const TextStyle(fontWeight: FontWeight.w700, fontSize: 14),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
