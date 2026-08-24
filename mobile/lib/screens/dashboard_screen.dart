import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:provider/provider.dart';

import '../services/mcp_service.dart';
import 'quick_actions_screen.dart';
import 'queue_builder_screen.dart';
import 'status_screen.dart';
import 'settings_screen.dart';

class DashboardScreen extends StatefulWidget {
  const DashboardScreen({super.key});
  @override
  State<DashboardScreen> createState() => _DashboardScreenState();
}

class _DashboardScreenState extends State<DashboardScreen> {
  int _tab = 0;

  final _screens = const [
    StatusScreen(),
    QuickActionsScreen(),
    QueueBuilderScreen(),
    SettingsScreen(),
  ];

  @override
  Widget build(BuildContext context) {
    final mcp = context.watch<McpService>();
    const border = Color(0xFF242632);

    return Scaffold(
      appBar: AppBar(
        title: Row(
          children: [
            Image.asset(
              'assets/icon.png',
              width: 26,
              height: 26,
              fit: BoxFit.contain,
              errorBuilder: (_, __, ___) => const Icon(Icons.timer_outlined, size: 22),
            ),
            const SizedBox(width: 10),
            const Text('AutoClick Remote'),
          ],
        ),
        actions: [
          Padding(
            padding: const EdgeInsets.only(right: 16),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Container(
                  width: 7,
                  height: 7,
                  decoration: BoxDecoration(
                    shape: BoxShape.circle,
                    color: mcp.isConnected ? const Color(0xFF10B981) : const Color(0xFFEF4444),
                  ),
                ),
                const SizedBox(width: 6),
                Text(
                  mcp.isConnected ? 'Connected' : 'Offline',
                  style: TextStyle(
                    fontSize: 12,
                    fontWeight: FontWeight.w500,
                    color: mcp.isConnected ? const Color(0xFF10B981) : const Color(0xFFEF4444),
                  ),
                ),
              ],
            ),
          ),
        ],
        bottom: PreferredSize(
          preferredSize: const Size.fromHeight(1),
          child: Container(color: border, height: 1),
        ),
      ),
      body: IndexedStack(index: _tab, children: _screens),
      bottomNavigationBar: Container(
        decoration: const BoxDecoration(
          color: Color(0xFF0C0D11),
          border: Border(top: BorderSide(color: border, width: 1)),
        ),
        child: NavigationBar(
          selectedIndex: _tab,
          onDestinationSelected: (i) {
            if (_tab != i) {
              HapticFeedback.selectionClick();
              setState(() => _tab = i);
            }
          },
          backgroundColor: Colors.transparent,
          indicatorColor: const Color(0xFF242632),
          height: 60,
          destinations: const [
            NavigationDestination(
              icon: Icon(Icons.dashboard_outlined, size: 20),
              selectedIcon: Icon(Icons.dashboard, size: 20, color: Color(0xFF3B82F6)),
              label: 'Status',
            ),
            NavigationDestination(
              icon: Icon(Icons.flash_on_outlined, size: 20),
              selectedIcon: Icon(Icons.flash_on, size: 20, color: Color(0xFF3B82F6)),
              label: 'Quick',
            ),
            NavigationDestination(
              icon: Icon(Icons.format_list_bulleted_outlined, size: 20),
              selectedIcon: Icon(Icons.format_list_bulleted, size: 20, color: Color(0xFF3B82F6)),
              label: 'Queue',
            ),
            NavigationDestination(
              icon: Icon(Icons.tune_outlined, size: 20),
              selectedIcon: Icon(Icons.tune, size: 20, color: Color(0xFF3B82F6)),
              label: 'Tools',
            ),
          ],
        ),
      ),
    );
  }
}
