import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

class LockoutSafeguardDialog extends StatefulWidget {
  final int initialDelaySeconds;

  const LockoutSafeguardDialog({
    super.key,
    this.initialDelaySeconds = 10,
  });

  static Future<int?> show(BuildContext context, {int initialDelaySeconds = 10}) {
    return showDialog<int>(
      context: context,
      builder: (_) => LockoutSafeguardDialog(initialDelaySeconds: initialDelaySeconds),
    );
  }

  @override
  State<LockoutSafeguardDialog> createState() => _LockoutSafeguardDialogState();
}

class _LockoutSafeguardDialogState extends State<LockoutSafeguardDialog> {
  late int _delaySeconds;
  bool _confirmed = false;

  static const _delays = [
    ('5s', 5),
    ('10s (Recommended)', 10),
    ('30s', 30),
    ('60s', 60),
  ];

  @override
  void initState() {
    super.initState();
    _delaySeconds = widget.initialDelaySeconds;
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      backgroundColor: const Color(0xFF14151E),
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(16),
        side: const BorderSide(color: Color(0xFFEF4444), width: 1.2),
      ),
      titlePadding: const EdgeInsets.fromLTRB(20, 20, 20, 0),
      contentPadding: const EdgeInsets.fromLTRB(20, 16, 20, 16),
      actionsPadding: const EdgeInsets.fromLTRB(16, 0, 16, 16),
      title: Row(
        children: [
          Container(
            padding: const EdgeInsets.all(8),
            decoration: BoxDecoration(
              color: const Color(0xFFEF4444).withValues(alpha: 0.15),
              borderRadius: BorderRadius.circular(8),
            ),
            child: const Icon(Icons.warning_amber_rounded, color: Color(0xFFEF4444), size: 24),
          ),
          const SizedBox(width: 12),
          const Expanded(
            child: Text(
              'Remote Lockout Warning',
              style: TextStyle(
                fontSize: 16,
                fontWeight: FontWeight.bold,
                color: Color(0xFFF1F5F9),
              ),
            ),
          ),
        ],
      ),
      content: SingleChildScrollView(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Container(
              padding: const EdgeInsets.all(12),
              decoration: BoxDecoration(
                color: const Color(0xFFEF4444).withValues(alpha: 0.1),
                borderRadius: BorderRadius.circular(8),
                border: Border.all(color: const Color(0xFFEF4444).withValues(alpha: 0.3)),
              ),
              child: const Text(
                'Shutting down the host PC powers off the system completely.\n\nTailscale and remote access will be TERMINATED. You cannot wake the PC remotely until someone presses the physical power button.',
                style: TextStyle(fontSize: 12, color: Color(0xFFFCA5A5), height: 1.4),
              ),
            ),
            const SizedBox(height: 16),
            const Text(
              'DISCONNECT GRACE DELAY',
              style: TextStyle(
                fontSize: 11,
                fontWeight: FontWeight.w700,
                letterSpacing: 0.8,
                color: Color(0xFF6B7280),
              ),
            ),
            const SizedBox(height: 8),
            ..._delays.map((d) {
              final selected = _delaySeconds == d.$2;
              return Padding(
                padding: const EdgeInsets.only(bottom: 6),
                child: InkWell(
                  onTap: () {
                    HapticFeedback.selectionClick();
                    setState(() => _delaySeconds = d.$2);
                  },
                  borderRadius: BorderRadius.circular(8),
                  child: Container(
                    padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
                    decoration: BoxDecoration(
                      color: selected ? const Color(0xFFEF4444).withValues(alpha: 0.2) : const Color(0xFF1A1D2B),
                      borderRadius: BorderRadius.circular(8),
                      border: Border.all(
                        color: selected ? const Color(0xFFEF4444) : const Color(0xFF242632),
                      ),
                    ),
                    child: Row(
                      mainAxisAlignment: MainAxisAlignment.spaceBetween,
                      children: [
                        Text(
                          d.$1,
                          style: TextStyle(
                            fontSize: 13,
                            fontWeight: selected ? FontWeight.bold : FontWeight.w500,
                            color: selected ? const Color(0xFFF1F5F9) : const Color(0xFF8B92A5),
                          ),
                        ),
                        if (selected)
                          const Icon(Icons.check, size: 16, color: Color(0xFFEF4444)),
                      ],
                    ),
                  ),
                ),
              );
            }),
            const SizedBox(height: 12),
            InkWell(
              onTap: () {
                HapticFeedback.selectionClick();
                setState(() => _confirmed = !_confirmed);
              },
              borderRadius: BorderRadius.circular(8),
              child: Row(
                children: [
                  Checkbox(
                    value: _confirmed,
                    activeColor: const Color(0xFFEF4444),
                    onChanged: (v) => setState(() => _confirmed = v ?? false),
                  ),
                  const Expanded(
                    child: Text(
                      'I understand this will shut down the remote PC',
                      style: TextStyle(fontSize: 12, color: Color(0xFFCBD5E1)),
                    ),
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.pop(context),
          child: const Text('Cancel', style: TextStyle(color: Color(0xFF8B92A5))),
        ),
        ElevatedButton(
          onPressed: !_confirmed
              ? null
              : () {
                  HapticFeedback.mediumImpact();
                  Navigator.pop(context, _delaySeconds);
                },
          style: ElevatedButton.styleFrom(
            backgroundColor: const Color(0xFFEF4444),
            foregroundColor: Colors.white,
          ),
          child: Text('Shut Down PC (${_delaySeconds}s)'),
        ),
      ],
    );
  }
}
