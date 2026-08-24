import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:autoclicktimer_remote/main.dart';

void main() {
  testWidgets('AppShell smoke test', (WidgetTester tester) async {
    SharedPreferences.setMockInitialValues({});
    final prefs = await SharedPreferences.getInstance();
    await tester.pumpWidget(AutoClickRemoteApp(prefs: prefs));
    expect(find.text('AutoClick Remote'), findsWidgets);
  });
}
