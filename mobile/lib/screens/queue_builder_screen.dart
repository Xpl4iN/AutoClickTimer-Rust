import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:provider/provider.dart';

import '../services/mcp_service.dart';
import '../widgets/duration_picker.dart';

class QueueBuilderScreen extends StatefulWidget {
  const QueueBuilderScreen({super.key});
  @override
  State<QueueBuilderScreen> createState() => _QueueBuilderScreenState();
}

class _QueueBuilderScreenState extends State<QueueBuilderScreen> {
  final _steps = <Map<String, dynamic>>[];
  int _repeat = 1;
  bool _loading = false;

  void _addStep(String action, int durationSeconds, [Map<String, dynamic>? opts]) {
    HapticFeedback.lightImpact();
    setState(() {
      _steps.add({
        'action': action,
        'after': durationSeconds,
        'label': action.toUpperCase(),
        ...?opts,
      });
    });
  }

  void _removeStep(int i) {
    HapticFeedback.selectionClick();
    setState(() => _steps.removeAt(i));
  }

  void _reorderItem(int oldIndex, int newIndex) {
    HapticFeedback.selectionClick();
    setState(() {
      final step = _steps.removeAt(oldIndex);
      _steps.insert(newIndex, step);
    });
  }

  Future<void> _run() async {
    if (_steps.isEmpty) return;
    HapticFeedback.mediumImpact();
    final mcp = context.read<McpService>();
    setState(() => _loading = true);
    try {
      await mcp.scheduleQueue(_steps, _repeat);
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Row(
              children: [
                const Icon(Icons.play_circle_fill, color: Colors.white, size: 18),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    'Queue started (${_steps.length} steps, ${_repeat == 0 ? "Infinite" : "${_repeat}x"})',
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
      if (mounted) setState(() => _loading = false);
    }
  }

  Future<void> _showAddDialog() async {
    String selectedAction = 'click';
    int stepDelay = 5;
    int sleepDuration = 3600; // 1 hour default for sleep
    int preSleepGrace = 5;
    String prompt = '';
    bool customSleep = false;
    final hoursCtrl = TextEditingController(text: '1');
    final minsCtrl = TextEditingController(text: '0');

    final actionOptions = [
      ('click', 'Mouse Click', Icons.mouse, const Color(0xFF38BDF8), 'Clicks at cursor position'),
      ('enter', 'Enter Key', Icons.keyboard_return, const Color(0xFF3B82F6), 'Sends Return keystroke'),
      ('type', 'Type Text', Icons.keyboard, const Color(0xFF06B6D4), 'Types custom text string'),
      ('sleep', 'Sleep PC (RTC Wake)', Icons.bedtime, const Color(0xFFA855F7), 'Sleeps PC & hardware-wakes later'),
      ('caffeine', 'Caffeine', Icons.coffee, const Color(0xFFF59E0B), 'Keeps PC awake for duration'),
      ('shutdown', 'Shut Down PC', Icons.power_settings_new, const Color(0xFFEF4444), 'Powers off system'),
    ];

    final sleepPresets = [
      ('15m', 15 * 60),
      ('30m', 30 * 60),
      ('1h', 1 * 3600),
      ('2h', 2 * 3600),
      ('4h', 4 * 3600),
      ('8h', 8 * 3600),
    ];

    await showModalBottomSheet(
      context: context,
      isScrollControlled: true,
      backgroundColor: const Color(0xFF12141C),
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(20)),
      ),
      builder: (_) => StatefulBuilder(
        builder: (ctx, setS) {
          final bottomInset = MediaQuery.of(ctx).viewInsets.bottom;
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

                  // Modal Header
                  Row(
                    mainAxisAlignment: MainAxisAlignment.spaceBetween,
                    children: [
                      const Text(
                        'Add Queue Step',
                        style: TextStyle(fontSize: 17, fontWeight: FontWeight.w700, color: Color(0xFFF1F5F9)),
                      ),
                      IconButton(
                        icon: const Icon(Icons.close, size: 20, color: Color(0xFF8B92A5)),
                        onPressed: () => Navigator.pop(context),
                      ),
                    ],
                  ),
                  const SizedBox(height: 12),

                  // Visual Action Selector Grid (replaces ugly dropdown)
                  const Text(
                    'CHOOSE ACTION',
                    style: TextStyle(
                      fontSize: 11,
                      fontWeight: FontWeight.w700,
                      letterSpacing: 0.8,
                      color: Color(0xFF6B7280),
                    ),
                  ),
                  const SizedBox(height: 8),

                  GridView.builder(
                    shrinkWrap: true,
                    physics: const NeverScrollableScrollPhysics(),
                    gridDelegate: const SliverGridDelegateWithFixedCrossAxisCount(
                      crossAxisCount: 2,
                      crossAxisSpacing: 8,
                      mainAxisSpacing: 8,
                      childAspectRatio: 2.3,
                    ),
                    itemCount: actionOptions.length,
                    itemBuilder: (_, idx) {
                      final opt = actionOptions[idx];
                      final isSelected = selectedAction == opt.$1;
                      return InkWell(
                        onTap: () {
                          HapticFeedback.selectionClick();
                          setS(() => selectedAction = opt.$1);
                        },
                        borderRadius: BorderRadius.circular(10),
                        child: AnimatedContainer(
                          duration: const Duration(milliseconds: 150),
                          padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
                          decoration: BoxDecoration(
                            color: isSelected ? opt.$4.withValues(alpha: 0.15) : const Color(0xFF181B26),
                            borderRadius: BorderRadius.circular(10),
                            border: Border.all(
                              color: isSelected ? opt.$4 : border,
                              width: isSelected ? 1.5 : 1,
                            ),
                          ),
                          child: Row(
                            children: [
                              Icon(opt.$3, size: 18, color: isSelected ? opt.$4 : const Color(0xFF8B92A5)),
                              const SizedBox(width: 8),
                              Expanded(
                                child: Column(
                                  mainAxisAlignment: MainAxisAlignment.center,
                                  crossAxisAlignment: CrossAxisAlignment.start,
                                  children: [
                                    Text(
                                      opt.$2,
                                      style: TextStyle(
                                        fontSize: 12,
                                        fontWeight: isSelected ? FontWeight.bold : FontWeight.w600,
                                        color: isSelected ? const Color(0xFFF1F5F9) : const Color(0xFF94A3B8),
                                      ),
                                      overflow: TextOverflow.ellipsis,
                                    ),
                                    Text(
                                      opt.$5,
                                      style: const TextStyle(fontSize: 9.5, color: Color(0xFF64748B)),
                                      overflow: TextOverflow.ellipsis,
                                    ),
                                  ],
                                ),
                              ),
                            ],
                          ),
                        ),
                      );
                    },
                  ),

                  const SizedBox(height: 16),

                  // Conditional Settings per Action
                  if (selectedAction == 'sleep') ...[
                    // Sleep Duration Configuration
                    Container(
                      padding: const EdgeInsets.all(14),
                      decoration: BoxDecoration(
                        color: const Color(0xFF181B26),
                        borderRadius: BorderRadius.circular(12),
                        border: Border.all(color: const Color(0xFFA855F7).withValues(alpha: 0.3)),
                      ),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          const Row(
                            children: [
                              Icon(Icons.bedtime, size: 16, color: Color(0xFFA855F7)),
                              SizedBox(width: 6),
                              Text(
                                'SLEEP DURATION (RTC WAKE)',
                                style: TextStyle(
                                  fontSize: 11,
                                  fontWeight: FontWeight.w700,
                                  letterSpacing: 0.6,
                                  color: Color(0xFFA855F7),
                                ),
                              ),
                            ],
                          ),
                          const SizedBox(height: 8),
                          Wrap(
                            spacing: 6,
                            runSpacing: 6,
                            children: [
                              ...sleepPresets.map((p) {
                                final selected = !customSleep && sleepDuration == p.$2;
                                return InkWell(
                                  onTap: () {
                                    HapticFeedback.selectionClick();
                                    setS(() {
                                      sleepDuration = p.$2;
                                      customSleep = false;
                                      hoursCtrl.text = (p.$2 ~/ 3600).toString();
                                      minsCtrl.text = ((p.$2 % 3600) ~/ 60).toString();
                                    });
                                  },
                                  borderRadius: BorderRadius.circular(8),
                                  child: Container(
                                    padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
                                    decoration: BoxDecoration(
                                      color: selected ? const Color(0xFFA855F7) : const Color(0xFF12141C),
                                      borderRadius: BorderRadius.circular(8),
                                      border: Border.all(color: selected ? const Color(0xFFA855F7) : border),
                                    ),
                                    child: Text(
                                      p.$1,
                                      style: TextStyle(
                                        fontSize: 12,
                                        fontWeight: selected ? FontWeight.bold : FontWeight.w500,
                                        color: selected ? Colors.white : const Color(0xFFCBD5E1),
                                      ),
                                    ),
                                  ),
                                );
                              }),
                              InkWell(
                                onTap: () => setS(() => customSleep = true),
                                borderRadius: BorderRadius.circular(8),
                                child: Container(
                                  padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
                                  decoration: BoxDecoration(
                                    color: customSleep ? const Color(0xFFA855F7) : const Color(0xFF12141C),
                                    borderRadius: BorderRadius.circular(8),
                                    border: Border.all(color: customSleep ? const Color(0xFFA855F7) : border),
                                  ),
                                  child: Text(
                                    'Custom',
                                    style: TextStyle(
                                      fontSize: 12,
                                      fontWeight: customSleep ? FontWeight.bold : FontWeight.w500,
                                      color: customSleep ? Colors.white : const Color(0xFFCBD5E1),
                                    ),
                                  ),
                                ),
                              ),
                            ],
                          ),
                          if (customSleep) ...[
                            const SizedBox(height: 10),
                            Row(
                              children: [
                                Expanded(
                                  child: TextField(
                                    controller: hoursCtrl,
                                    keyboardType: TextInputType.number,
                                    decoration: const InputDecoration(
                                      labelText: 'Hours',
                                      contentPadding: EdgeInsets.symmetric(horizontal: 10, vertical: 8),
                                    ),
                                    onChanged: (_) {
                                      final h = int.tryParse(hoursCtrl.text.trim()) ?? 0;
                                      final m = int.tryParse(minsCtrl.text.trim()) ?? 0;
                                      setS(() => sleepDuration = (h * 3600) + (m * 60));
                                    },
                                  ),
                                ),
                                const SizedBox(width: 8),
                                Expanded(
                                  child: TextField(
                                    controller: minsCtrl,
                                    keyboardType: TextInputType.number,
                                    decoration: const InputDecoration(
                                      labelText: 'Minutes',
                                      contentPadding: EdgeInsets.symmetric(horizontal: 10, vertical: 8),
                                    ),
                                    onChanged: (_) {
                                      final h = int.tryParse(hoursCtrl.text.trim()) ?? 0;
                                      final m = int.tryParse(minsCtrl.text.trim()) ?? 0;
                                      setS(() => sleepDuration = (h * 3600) + (m * 60));
                                    },
                                  ),
                                ),
                              ],
                            ),
                          ],
                          const SizedBox(height: 12),
                          const Text(
                            'PRE-SLEEP GRACE DELAY (SECONDS)',
                            style: TextStyle(fontSize: 10, fontWeight: FontWeight.w700, color: Color(0xFF6B7280)),
                          ),
                          const SizedBox(height: 6),
                          DurationPicker(
                            value: preSleepGrace,
                            onChanged: (v) => setS(() => preSleepGrace = v),
                          ),
                        ],
                      ),
                    ),
                  ] else if (selectedAction == 'type') ...[
                    // Type Text prompt input
                    TextField(
                      decoration: const InputDecoration(
                        labelText: 'Text to type remotely',
                        hintText: 'Enter string or keystroke sequence...',
                      ),
                      onChanged: (v) => prompt = v,
                    ),
                    const SizedBox(height: 14),
                    const Text(
                      'STEP DELAY (BEFORE TYPING)',
                      style: TextStyle(fontSize: 11, fontWeight: FontWeight.w700, letterSpacing: 0.8, color: Color(0xFF6B7280)),
                    ),
                    const SizedBox(height: 8),
                    DurationPicker(
                      value: stepDelay,
                      onChanged: (v) => setS(() => stepDelay = v),
                    ),
                  ] else if (selectedAction == 'shutdown') ...[
                    Container(
                      padding: const EdgeInsets.all(12),
                      decoration: BoxDecoration(
                        color: const Color(0xFFEF4444).withValues(alpha: 0.1),
                        borderRadius: BorderRadius.circular(8),
                        border: Border.all(color: const Color(0xFFEF4444).withValues(alpha: 0.3)),
                      ),
                      child: const Row(
                        children: [
                          Icon(Icons.warning_amber_rounded, size: 16, color: Color(0xFFEF4444)),
                          SizedBox(width: 8),
                          Expanded(
                            child: Text(
                              'Warning: Shutdown powers off the PC. You cannot remote in again until physically turned on.',
                              style: TextStyle(fontSize: 11, color: Color(0xFFFCA5A5)),
                            ),
                          ),
                        ],
                      ),
                    ),
                    const SizedBox(height: 14),
                    const Text(
                      'DELAY BEFORE SHUTDOWN',
                      style: TextStyle(fontSize: 11, fontWeight: FontWeight.w700, letterSpacing: 0.8, color: Color(0xFF6B7280)),
                    ),
                    const SizedBox(height: 8),
                    DurationPicker(
                      value: stepDelay,
                      onChanged: (v) => setS(() => stepDelay = v),
                    ),
                  ] else ...[
                    const Text(
                      'STEP DELAY (WAIT TIME BEFORE EXECUTION)',
                      style: TextStyle(
                        fontSize: 11,
                        fontWeight: FontWeight.w700,
                        letterSpacing: 0.8,
                        color: Color(0xFF6B7280),
                      ),
                    ),
                    const SizedBox(height: 8),
                    DurationPicker(
                      value: stepDelay,
                      onChanged: (v) => setS(() => stepDelay = v),
                    ),
                  ],

                  const SizedBox(height: 20),

                  ElevatedButton(
                    onPressed: () {
                      Navigator.pop(context);
                      if (selectedAction == 'sleep') {
                        _addStep(
                          'sleep',
                          sleepDuration,
                          {'pre_sleep_grace': preSleepGrace},
                        );
                      } else {
                        _addStep(
                          selectedAction,
                          stepDelay,
                          selectedAction == 'type' && prompt.isNotEmpty ? {'prompt': prompt} : null,
                        );
                      }
                    },
                    style: ElevatedButton.styleFrom(
                      padding: const EdgeInsets.symmetric(vertical: 14),
                      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
                    ),
                    child: Text(
                      selectedAction == 'sleep'
                          ? 'Add Sleep Step (${_fmtSecs(sleepDuration)})'
                          : 'Add Step to Queue',
                    ),
                  ),
                ],
              ),
            ),
          );
        },
      ),
    );

    hoursCtrl.dispose();
    minsCtrl.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: Colors.transparent,
      body: Column(
        children: [
          // Repeat Count Bar
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 12, 16, 0),
            child: Container(
              padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
              decoration: BoxDecoration(
                color: const Color(0xFF14151E),
                borderRadius: BorderRadius.circular(12),
                border: Border.all(color: const Color(0xFF242838)),
              ),
              child: Row(
                children: [
                  const Icon(Icons.repeat, size: 18, color: Color(0xFF8B92A5)),
                  const SizedBox(width: 8),
                  const Text('Repeat Count', style: TextStyle(fontWeight: FontWeight.w600, fontSize: 13.5)),
                  const Spacer(),
                  IconButton(
                    icon: const Icon(Icons.remove, size: 18),
                    onPressed: () {
                      HapticFeedback.selectionClick();
                      setState(() => _repeat = (_repeat - 1).clamp(0, 99));
                    },
                  ),
                  Container(
                    padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
                    decoration: BoxDecoration(
                      color: const Color(0xFF1E2232),
                      borderRadius: BorderRadius.circular(6),
                    ),
                    child: Text(
                      _repeat == 0 ? '∞ (Infinite)' : '$_repeat',
                      style: const TextStyle(fontSize: 14, fontWeight: FontWeight.bold, fontFamily: 'monospace'),
                    ),
                  ),
                  IconButton(
                    icon: const Icon(Icons.add, size: 18),
                    onPressed: () {
                      HapticFeedback.selectionClick();
                      setState(() => _repeat = (_repeat + 1).clamp(0, 99));
                    },
                  ),
                ],
              ),
            ),
          ),

          // Steps list
          Expanded(
            child: _steps.isEmpty
                ? Center(
                    child: Column(
                      mainAxisAlignment: MainAxisAlignment.center,
                      children: [
                        Container(
                          padding: const EdgeInsets.all(16),
                          decoration: BoxDecoration(
                            color: const Color(0xFF14151E),
                            shape: BoxShape.circle,
                            border: Border.all(color: const Color(0xFF242838)),
                          ),
                          child: const Icon(Icons.playlist_add, size: 36, color: Color(0xFF52586B)),
                        ),
                        const SizedBox(height: 12),
                        const Text('Queue is empty', style: TextStyle(color: Color(0xFFCBD5E1), fontSize: 15, fontWeight: FontWeight.w600)),
                        const SizedBox(height: 4),
                        const Text('Add sequential clicks, sleep cycles, and keystrokes', style: TextStyle(color: Color(0xFF8B92A5), fontSize: 12)),
                      ],
                    ),
                  )
                : ReorderableListView.builder(
                    padding: const EdgeInsets.all(16),
                    itemCount: _steps.length,
                    onReorderItem: _reorderItem,
                    itemBuilder: (_, i) {
                      final step = _steps[i];
                      final action = step['action'] as String;
                      final isSleep = action == 'sleep';
                      final accent = isSleep
                          ? const Color(0xFFA855F7)
                          : action == 'shutdown'
                              ? const Color(0xFFEF4444)
                              : const Color(0xFF3B82F6);

                      return Container(
                        key: ValueKey('step_$i'),
                        margin: const EdgeInsets.only(bottom: 8),
                        decoration: BoxDecoration(
                          color: const Color(0xFF14151E),
                          borderRadius: BorderRadius.circular(12),
                          border: Border.all(color: const Color(0xFF242838)),
                        ),
                        child: ListTile(
                          dense: true,
                          contentPadding: const EdgeInsets.symmetric(horizontal: 14, vertical: 4),
                          leading: Container(
                            width: 28,
                            height: 28,
                            decoration: BoxDecoration(
                              color: accent.withValues(alpha: 0.15),
                              borderRadius: BorderRadius.circular(8),
                            ),
                            child: Center(
                              child: Text(
                                '${i + 1}',
                                style: TextStyle(color: accent, fontWeight: FontWeight.bold, fontSize: 12),
                              ),
                            ),
                          ),
                          title: Text(
                            isSleep ? 'SLEEP PC (RTC WAKE)' : action.toUpperCase(),
                            style: const TextStyle(fontWeight: FontWeight.w700, fontSize: 13),
                          ),
                          subtitle: Text(
                            isSleep
                                ? 'Sleeps for ${_fmtSecs(step['after'] as int)} (hardware wake armed)'
                                : '${_fmtSecs(step['after'] as int)} delay${step['prompt'] != null ? ' • "${step['prompt']}"' : ''}',
                            style: const TextStyle(color: Color(0xFF8B92A5), fontSize: 11.5),
                          ),
                          trailing: Row(
                            mainAxisSize: MainAxisSize.min,
                            children: [
                              IconButton(
                                icon: const Icon(Icons.close, size: 16, color: Color(0xFF8B92A5)),
                                onPressed: () => _removeStep(i),
                              ),
                              const Icon(Icons.drag_indicator, size: 18, color: Color(0xFF52586B)),
                            ],
                          ),
                        ),
                      );
                    },
                  ),
          ),

          // Bottom Action Buttons
          Container(
            padding: const EdgeInsets.fromLTRB(16, 10, 16, 16),
            decoration: const BoxDecoration(
              color: Color(0xFF0C0D11),
              border: Border(top: BorderSide(color: Color(0xFF242838))),
            ),
            child: Row(
              children: [
                Expanded(
                  child: OutlinedButton.icon(
                    onPressed: _showAddDialog,
                    icon: const Icon(Icons.add, size: 16),
                    label: const Text('Add Step', style: TextStyle(fontSize: 13.5)),
                    style: OutlinedButton.styleFrom(
                      side: const BorderSide(color: Color(0xFF242838)),
                      backgroundColor: const Color(0xFF14151E),
                      padding: const EdgeInsets.symmetric(vertical: 13),
                      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
                    ),
                  ),
                ),
                if (_steps.isNotEmpty) ...[
                  const SizedBox(width: 10),
                  Expanded(
                    child: ElevatedButton.icon(
                      onPressed: _loading ? null : _run,
                      icon: const Icon(Icons.play_arrow, size: 18),
                      label: Text(_loading ? 'Starting...' : 'Run Queue', style: const TextStyle(fontSize: 13.5)),
                      style: ElevatedButton.styleFrom(
                        backgroundColor: const Color(0xFF10B981),
                        padding: const EdgeInsets.symmetric(vertical: 13),
                        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
                      ),
                    ),
                  ),
                ],
              ],
            ),
          ),
        ],
      ),
    );
  }

  String _fmtSecs(int s) {
    if (s >= 3600) {
      final h = s ~/ 3600;
      final m = (s % 3600) ~/ 60;
      return m > 0 ? '${h}h ${m}m' : '${h}h';
    }
    if (s >= 60) return '${s ~/ 60}m';
    return '${s}s';
  }
}
