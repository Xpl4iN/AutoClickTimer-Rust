import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:provider/provider.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'services/mcp_service.dart';
import 'screens/connect_screen.dart';
import 'screens/dashboard_screen.dart';

void main() async {
  WidgetsFlutterBinding.ensureInitialized();
  SystemChrome.setSystemUIOverlayStyle(
    const SystemUiOverlayStyle(
      statusBarColor: Colors.transparent,
      statusBarIconBrightness: Brightness.light,
      systemNavigationBarColor: Color(0xFF090A0F),
      systemNavigationBarIconBrightness: Brightness.light,
    ),
  );
  final prefs = await SharedPreferences.getInstance();
  runApp(AutoClickRemoteApp(prefs: prefs));
}

class AutoClickRemoteApp extends StatelessWidget {
  final SharedPreferences prefs;
  const AutoClickRemoteApp({super.key, required this.prefs});

  @override
  Widget build(BuildContext context) {
    return ChangeNotifierProvider(
      create: (_) => McpService(
        host: prefs.getString('host') ?? '',
        port: prefs.getInt('port') ?? 7890,
        apiKey: prefs.getString('apiKey'),
      ),
      child: MaterialApp(
        title: 'AutoClick Remote',
        debugShowCheckedModeBanner: false,
        themeMode: ThemeMode.dark,
        darkTheme: _buildCleanPrecisionTheme(),
        theme: _buildCleanPrecisionTheme(),
        home: const AppShell(),
      ),
    );
  }

  ThemeData _buildCleanPrecisionTheme() {
    const bg = Color(0xFF090A0F);
    const surface = Color(0xFF12141E);
    const cardBg = Color(0xFF14151E);
    const border = Color(0xFF242838);
    const primary = Color(0xFF3B82F6);
    const textPrimary = Color(0xFFF1F5F9);
    const textSecondary = Color(0xFF8B92A5);

    return ThemeData(
      useMaterial3: true,
      brightness: Brightness.dark,
      scaffoldBackgroundColor: bg,
      colorScheme: const ColorScheme.dark(
        primary: primary,
        secondary: Color(0xFF64748B),
        surface: surface,
        surfaceContainerHighest: cardBg,
        onPrimary: Colors.white,
        onSurface: textPrimary,
        onSurfaceVariant: textSecondary,
        error: Color(0xFFEF4444),
      ),
      cardTheme: CardThemeData(
        color: cardBg,
        elevation: 0,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(14),
          side: const BorderSide(color: border, width: 1),
        ),
      ),
      elevatedButtonTheme: ElevatedButtonThemeData(
        style: ElevatedButton.styleFrom(
          backgroundColor: primary,
          foregroundColor: Colors.white,
          elevation: 0,
          shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
          padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 14),
          textStyle: const TextStyle(fontWeight: FontWeight.w600, fontSize: 14, letterSpacing: -0.1),
        ),
      ),
      inputDecorationTheme: InputDecorationTheme(
        filled: true,
        fillColor: const Color(0xFF181A26),
        contentPadding: const EdgeInsets.symmetric(horizontal: 16, vertical: 14),
        border: OutlineInputBorder(
          borderRadius: BorderRadius.circular(10),
          borderSide: const BorderSide(color: border),
        ),
        enabledBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(10),
          borderSide: const BorderSide(color: border),
        ),
        focusedBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(10),
          borderSide: const BorderSide(color: primary, width: 1.5),
        ),
        labelStyle: const TextStyle(color: textSecondary, fontSize: 13),
        hintStyle: const TextStyle(color: Color(0xFF52586B), fontSize: 13),
      ),
      appBarTheme: const AppBarTheme(
        backgroundColor: bg,
        elevation: 0,
        scrolledUnderElevation: 0,
        centerTitle: false,
        titleTextStyle: TextStyle(
          color: textPrimary,
          fontSize: 16,
          fontWeight: FontWeight.w700,
          letterSpacing: -0.2,
        ),
        iconTheme: IconThemeData(color: textPrimary),
      ),
    );
  }
}

class AppShell extends StatelessWidget {
  const AppShell({super.key});

  @override
  Widget build(BuildContext context) {
    final mcp = context.watch<McpService>();
    return mcp.isConnected ? const DashboardScreen() : const ConnectScreen();
  }
}
