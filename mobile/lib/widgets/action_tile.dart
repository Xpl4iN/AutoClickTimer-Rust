import 'package:flutter/material.dart';

class ActionTile extends StatelessWidget {
  final String title;
  final String subtitle;
  final IconData icon;
  final VoidCallback? onTap;
  final bool disabled;
  final Color? accentColor;
  final String? badgeText;

  const ActionTile({
    super.key,
    required this.title,
    required this.subtitle,
    required this.icon,
    this.onTap,
    this.disabled = false,
    this.accentColor,
    this.badgeText,
  });

  @override
  Widget build(BuildContext context) {
    final accent = accentColor ?? const Color(0xFF3B82F6);

    return Container(
      margin: const EdgeInsets.only(bottom: 8),
      decoration: BoxDecoration(
        color: const Color(0xFF14151E),
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: const Color(0xFF242838)),
      ),
      child: Material(
        color: Colors.transparent,
        child: InkWell(
          borderRadius: BorderRadius.circular(12),
          onTap: disabled ? null : onTap,
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
            child: Row(
              children: [
                Container(
                  padding: const EdgeInsets.all(8),
                  decoration: BoxDecoration(
                    color: accent.withValues(alpha: 0.12),
                    borderRadius: BorderRadius.circular(10),
                    border: Border.all(color: accent.withValues(alpha: 0.25)),
                  ),
                  child: Icon(icon, color: accent, size: 20),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Row(
                        children: [
                          Flexible(
                            child: Text(
                              title,
                              style: const TextStyle(
                                fontWeight: FontWeight.w600,
                                fontSize: 13.5,
                                color: Color(0xFFF1F5F9),
                              ),
                              overflow: TextOverflow.ellipsis,
                            ),
                          ),
                          if (badgeText != null) ...[
                            const SizedBox(width: 6),
                            Container(
                              padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 1.5),
                              decoration: BoxDecoration(
                                color: accent.withValues(alpha: 0.15),
                                borderRadius: BorderRadius.circular(4),
                                border: Border.all(color: accent.withValues(alpha: 0.3)),
                              ),
                              child: Text(
                                badgeText!,
                                style: TextStyle(
                                  fontSize: 9,
                                  fontWeight: FontWeight.w700,
                                  letterSpacing: 0.4,
                                  color: accent,
                                ),
                              ),
                            ),
                          ],
                        ],
                      ),
                      const SizedBox(height: 2),
                      Text(
                        subtitle,
                        style: const TextStyle(color: Color(0xFF8B92A5), fontSize: 11),
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ],
                  ),
                ),
                const SizedBox(width: 8),
                const Icon(Icons.arrow_forward_ios, color: Color(0xFF52586B), size: 12),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
