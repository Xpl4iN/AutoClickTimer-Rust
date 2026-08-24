import 'package:flutter/material.dart';

class ActionChipRow extends StatelessWidget {
  final void Function(String action) onQuickAction;
  const ActionChipRow({super.key, required this.onQuickAction});

  static const _actions = [
    ('Click', 'click', Icons.mouse, Color(0xFF38BDF8)),
    ('Enter', 'enter', Icons.keyboard_return, Color(0xFF3B82F6)),
    ('Sleep', 'sleep', Icons.bedtime_outlined, Color(0xFFA855F7)),
    ('Shutdown', 'shutdown', Icons.power_settings_new, Color(0xFFEF4444)),
  ];

  @override
  Widget build(BuildContext context) {
    return Row(
      children: _actions.map((a) {
        return Expanded(
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 3),
            child: OutlinedButton(
              onPressed: () => onQuickAction(a.$2),
              style: OutlinedButton.styleFrom(
                side: BorderSide(color: a.$4.withValues(alpha: 0.3)),
                backgroundColor: const Color(0xFF14151E),
                foregroundColor: const Color(0xFFF1F5F9),
                padding: const EdgeInsets.symmetric(vertical: 10),
                shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
              ),
              child: Row(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  Icon(a.$3, size: 14, color: a.$4),
                  const SizedBox(width: 4),
                  Text(
                    a.$1,
                    style: const TextStyle(fontSize: 11, fontWeight: FontWeight.w600),
                  ),
                ],
              ),
            ),
          ),
        );
      }).toList(),
    );
  }
}
