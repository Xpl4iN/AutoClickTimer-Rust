import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:provider/provider.dart';

import '../services/mcp_service.dart';

class SettingsScreen extends StatefulWidget {
  const SettingsScreen({super.key});
  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {
  bool _configLoading = false;

  Future<void> _disconnect() async {
    HapticFeedback.mediumImpact();
    final mcp = context.read<McpService>();
    await mcp.disconnect();
  }

  Future<void> _configureWakeLock() async {
    HapticFeedback.lightImpact();
    final mcp = context.read<McpService>();
    setState(() => _configLoading = true);
    try {
      final r = await mcp.configurePasswordlessWake();
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Row(
              children: [
                const Icon(Icons.check_circle, color: Colors.white, size: 18),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    r['message'] ?? 'Passwordless wake configured successfully',
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
          ),
        );
      }
    } finally {
      if (mounted) setState(() => _configLoading = false);
    }
  }

  Future<void> _showCursorPos() async {
    HapticFeedback.lightImpact();
    final mcp = context.read<McpService>();
    try {
      final r = await mcp.getCursorPos();
      if (mounted) {
        showDialog(
          context: context,
          builder: (_) => AlertDialog(
            backgroundColor: const Color(0xFF14151E),
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(16),
              side: const BorderSide(color: Color(0xFF242838)),
            ),
            title: const Row(
              children: [
                Icon(Icons.mouse, color: Color(0xFF38BDF8), size: 20),
                SizedBox(width: 8),
                Text('Current PC Cursor', style: TextStyle(fontSize: 16, fontWeight: FontWeight.bold)),
              ],
            ),
            content: Container(
              padding: const EdgeInsets.all(14),
              decoration: BoxDecoration(
                color: const Color(0xFF1E2232),
                borderRadius: BorderRadius.circular(10),
              ),
              child: Text(
                'X: ${r['x']} px\nY: ${r['y']} px',
                style: const TextStyle(fontSize: 16, fontFamily: 'monospace', fontWeight: FontWeight.w600, color: Color(0xFFF1F5F9)),
              ),
            ),
            actions: [
              TextButton(
                onPressed: () => Navigator.pop(context),
                child: const Text('Dismiss', style: TextStyle(color: Color(0xFF3B82F6))),
              ),
            ],
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
    final mcp = context.watch<McpService>();

    return SingleChildScrollView(
      padding: const EdgeInsets.all(16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const Text(
            'CONNECTION & TAILSCALE',
            style: TextStyle(
              fontSize: 11,
              fontWeight: FontWeight.w700,
              letterSpacing: 0.8,
              color: Color(0xFF6B7280),
            ),
          ),
          const SizedBox(height: 8),

          Container(
            decoration: BoxDecoration(
              color: const Color(0xFF14151E),
              borderRadius: BorderRadius.circular(14),
              border: Border.all(color: const Color(0xFF242838)),
            ),
            child: Column(
              children: [
                ListTile(
                  dense: true,
                  leading: Container(
                    padding: const EdgeInsets.all(6),
                    decoration: BoxDecoration(
                      color: const Color(0xFF3B82F6).withValues(alpha: 0.12),
                      borderRadius: BorderRadius.circular(8),
                    ),
                    child: const Icon(Icons.dns_outlined, color: Color(0xFF3B82F6), size: 18),
                  ),
                  title: const Text('Target Host Address', style: TextStyle(fontWeight: FontWeight.w600, fontSize: 13.5)),
                  subtitle: Text('${mcp.host}:${mcp.port}', style: const TextStyle(fontFamily: 'monospace', color: Color(0xFF8B92A5), fontSize: 12)),
                  trailing: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Container(
                        width: 8,
                        height: 8,
                        decoration: BoxDecoration(
                          shape: BoxShape.circle,
                          color: mcp.isConnected ? const Color(0xFF10B981) : const Color(0xFFEF4444),
                        ),
                      ),
                      const SizedBox(width: 6),
                      Text(
                        mcp.isConnected ? 'ONLINE' : 'OFFLINE',
                        style: TextStyle(
                          fontSize: 11,
                          fontWeight: FontWeight.w700,
                          color: mcp.isConnected ? const Color(0xFF10B981) : const Color(0xFFEF4444),
                        ),
                      ),
                    ],
                  ),
                ),
                const Divider(height: 1, color: Color(0xFF242838)),
                ListTile(
                  dense: true,
                  leading: Container(
                    padding: const EdgeInsets.all(6),
                    decoration: BoxDecoration(
                      color: const Color(0xFFEF4444).withValues(alpha: 0.12),
                      borderRadius: BorderRadius.circular(8),
                    ),
                    child: const Icon(Icons.logout, color: Color(0xFFEF4444), size: 18),
                  ),
                  title: const Text('Disconnect from Host', style: TextStyle(color: Color(0xFFEF4444), fontWeight: FontWeight.w600, fontSize: 13.5)),
                  onTap: _disconnect,
                ),
              ],
            ),
          ),

          const SizedBox(height: 22),

          const Text(
            'REMOTE RESILIENCE & WAKE SETTINGS',
            style: TextStyle(
              fontSize: 11,
              fontWeight: FontWeight.w700,
              letterSpacing: 0.8,
              color: Color(0xFF6B7280),
            ),
          ),
          const SizedBox(height: 8),

          Container(
            decoration: BoxDecoration(
              color: const Color(0xFF14151E),
              borderRadius: BorderRadius.circular(14),
              border: Border.all(color: const Color(0xFF242838)),
            ),
            child: Column(
              children: [
                ListTile(
                  dense: true,
                  leading: Container(
                    padding: const EdgeInsets.all(6),
                    decoration: BoxDecoration(
                      color: const Color(0xFFA855F7).withValues(alpha: 0.12),
                      borderRadius: BorderRadius.circular(8),
                    ),
                    child: const Icon(Icons.lock_open, color: Color(0xFFA855F7), size: 18),
                  ),
                  title: const Text('Configure Zero-Password Wake', style: TextStyle(fontWeight: FontWeight.w600, fontSize: 13.5)),
                  subtitle: const Text('Configures PC power scheme to resume unlocked after RTC sleep', style: TextStyle(color: Color(0xFF8B92A5), fontSize: 11.5)),
                  trailing: _configLoading
                      ? const SizedBox(width: 16, height: 16, child: CircularProgressIndicator(strokeWidth: 2, color: Color(0xFFA855F7)))
                      : const Icon(Icons.chevron_right, size: 18, color: Color(0xFF52586B)),
                  onTap: _configLoading ? null : _configureWakeLock,
                ),
                const Divider(height: 1, color: Color(0xFF242838)),
                ListTile(
                  dense: true,
                  leading: Container(
                    padding: const EdgeInsets.all(6),
                    decoration: BoxDecoration(
                      color: const Color(0xFF38BDF8).withValues(alpha: 0.12),
                      borderRadius: BorderRadius.circular(8),
                    ),
                    child: const Icon(Icons.ads_click, color: Color(0xFF38BDF8), size: 18),
                  ),
                  title: const Text('Inspect PC Cursor Position', style: TextStyle(fontWeight: FontWeight.w600, fontSize: 13.5)),
                  subtitle: const Text('Queries active X, Y coordinates on remote screen', style: TextStyle(color: Color(0xFF8B92A5), fontSize: 11.5)),
                  trailing: const Icon(Icons.chevron_right, size: 18, color: Color(0xFF52586B)),
                  onTap: _showCursorPos,
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
