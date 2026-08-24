import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

class DurationPicker extends StatelessWidget {
  final int value;
  final void Function(int) onChanged;
  final String label;

  const DurationPicker({
    super.key,
    required this.value,
    required this.onChanged,
    this.label = 'Delay',
  });

  static const _presets = [1, 2, 3, 5, 10, 15, 30, 60, 300, 1800, 3600];

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: 38,
      child: ListView.builder(
        scrollDirection: Axis.horizontal,
        itemCount: _presets.length,
        itemBuilder: (_, i) {
          final s = _presets[i];
          final selected = value == s;
          return Padding(
            padding: const EdgeInsets.only(right: 6),
            child: InkWell(
              onTap: () {
                HapticFeedback.selectionClick();
                onChanged(s);
              },
              borderRadius: BorderRadius.circular(8),
              child: AnimatedContainer(
                duration: const Duration(milliseconds: 150),
                padding: const EdgeInsets.symmetric(horizontal: 14),
                decoration: BoxDecoration(
                  color: selected ? const Color(0xFF3B82F6) : const Color(0xFF14151B),
                  borderRadius: BorderRadius.circular(8),
                  border: Border.all(
                    color: selected ? const Color(0xFF60A5FA) : const Color(0xFF242632),
                    width: selected ? 1.5 : 1,
                  ),
                  boxShadow: selected
                      ? [
                          BoxShadow(
                            color: const Color(0xFF3B82F6).withValues(alpha: 0.3),
                            blurRadius: 8,
                            offset: const Offset(0, 2),
                          )
                        ]
                      : null,
                ),
                child: Center(
                  child: Text(
                    _fmt(s),
                    style: TextStyle(
                      color: selected ? Colors.white : const Color(0xFF8B92A5),
                      fontWeight: selected ? FontWeight.w700 : FontWeight.w500,
                      fontSize: 12,
                      fontFamily: 'monospace',
                    ),
                  ),
                ),
              ),
            ),
          );
        },
      ),
    );
  }

  String _fmt(int s) {
    if (s >= 3600) return '${s ~/ 3600}h';
    if (s >= 60) return '${s ~/ 60}m';
    return '${s}s';
  }
}
