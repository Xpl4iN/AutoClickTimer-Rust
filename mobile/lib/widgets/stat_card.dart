import 'package:flutter/material.dart';

class StatCard extends StatelessWidget {
  final String label;
  final String value;

  const StatCard({super.key, required this.label, required this.value});

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
        color: const Color(0xFF14151B),
        borderRadius: BorderRadius.circular(10),
        border: Border.all(color: const Color(0xFF242632)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            label.toUpperCase(),
            style: const TextStyle(
              color: Color(0xFF6B7280),
              fontSize: 10,
              fontWeight: FontWeight.w700,
              letterSpacing: 0.6,
            ),
          ),
          const SizedBox(height: 4),
          Text(
            value,
            style: const TextStyle(
              fontWeight: FontWeight.w600,
              fontSize: 14,
              color: Color(0xFFF1F5F9),
            ),
            overflow: TextOverflow.ellipsis,
          ),
        ],
      ),
    );
  }
}
