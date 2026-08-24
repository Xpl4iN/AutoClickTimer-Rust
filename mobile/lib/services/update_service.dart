import 'dart:convert';
import 'dart:io';
import 'package:flutter/services.dart';

class UpdateInfo {
  final String version;
  final String tagName;
  final String downloadUrl;
  final String releaseNotes;
  final int sizeBytes;

  const UpdateInfo({
    required this.version,
    required this.tagName,
    required this.downloadUrl,
    required this.releaseNotes,
    required this.sizeBytes,
  });
}

class UpdateService {
  static const String currentVersion = '1.5.1';
  static const String repo = 'Xpl4iN/AutoClickTimer-Rust';
  static const MethodChannel _channel = MethodChannel('com.xp.autoclicktimer_remote/updater');

  /// Compare two semantic version strings (e.g., "1.5.2" > "1.5.1")
  static bool isNewer(String latest, String current) {
    final lClean = latest.replaceAll(RegExp(r'^v'), '').trim();
    final cClean = current.replaceAll(RegExp(r'^v'), '').trim();

    final lParts = lClean.split('.').map((e) => int.tryParse(e) ?? 0).toList();
    final cParts = cClean.split('.').map((e) => int.tryParse(e) ?? 0).toList();

    while (lParts.length < 3) lParts.add(0);
    while (cParts.length < 3) cParts.add(0);

    for (int i = 0; i < 3; i++) {
      if (lParts[i] > cParts[i]) return true;
      if (lParts[i] < cParts[i]) return false;
    }
    return false;
  }

  /// Check GitHub releases for a newer APK release
  static Future<UpdateInfo?> checkForUpdate() async {
    final client = HttpClient();
    client.connectionTimeout = const Duration(seconds: 10);
    try {
      final request = await client.getUrl(
        Uri.parse('https://api.github.com/repos/$repo/releases/latest'),
      );
      request.headers.set('User-Agent', 'AutoClickTimer-Remote/$currentVersion');
      request.headers.set('Accept', 'application/vnd.github.v3+json');

      final response = await request.close();
      if (response.statusCode != 200) return null;

      final body = await response.transform(utf8.decoder).join();
      final json = jsonDecode(body) as Map<String, dynamic>;

      final tagName = json['tag_name'] as String? ?? '';
      final version = tagName.replaceAll(RegExp(r'^v'), '');
      final releaseNotes = json['body'] as String? ?? '';

      if (!isNewer(version, currentVersion)) {
        return null; // Current version is up to date
      }

      final assets = json['assets'] as List<dynamic>? ?? [];
      for (final asset in assets) {
        final name = (asset['name'] as String? ?? '').toLowerCase();
        if (name.endsWith('.apk')) {
          final downloadUrl = asset['browser_download_url'] as String? ?? '';
          final size = asset['size'] as int? ?? 0;
          if (downloadUrl.isNotEmpty) {
            return UpdateInfo(
              version: version,
              tagName: tagName,
              downloadUrl: downloadUrl,
              releaseNotes: releaseNotes,
              sizeBytes: size,
            );
          }
        }
      }
      return null;
    } catch (_) {
      return null;
    } finally {
      client.close();
    }
  }

  /// Download the APK with progress and trigger the system installer
  static Future<void> downloadAndInstall(
    String downloadUrl, {
    required void Function(double progress) onProgress,
  }) async {
    final client = HttpClient();
    client.connectionTimeout = const Duration(seconds: 15);
    try {
      final request = await client.getUrl(Uri.parse(downloadUrl));
      request.headers.set('User-Agent', 'AutoClickTimer-Remote/$currentVersion');
      final response = await request.close();

      if (response.statusCode != 200) {
        throw Exception('Download failed with HTTP ${response.statusCode}');
      }

      final contentLength = response.contentLength;
      final tempDir = Directory.systemTemp;
      final file = File('${tempDir.path}/autoclicktimer-update.apk');

      if (await file.exists()) {
        await file.delete();
      }

      final sink = file.openWrite();
      int received = 0;

      await for (final chunk in response) {
        sink.add(chunk);
        received += chunk.length;
        if (contentLength > 0) {
          onProgress(received / contentLength);
        }
      }

      await sink.flush();
      await sink.close();

      // Trigger Android native package installer
      await _channel.invokeMethod('installApk', {'filePath': file.path});
    } finally {
      client.close();
    }
  }
}
