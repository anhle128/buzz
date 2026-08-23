import 'dart:convert';

import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:http/http.dart' as http;

import '../relay/relay_provider.dart';

final _relayPubkeyPattern = RegExp(r'^[0-9a-f]{64}$');

/// NIP-11 relay information used by community icons and App attribution.
class RelayInformation {
  /// Active relay pubkey from NIP-11 `self`, lowercase 64-hex, or null.
  final String? self;

  /// Optional community icon URL from NIP-11 `icon`.
  final String? icon;

  const RelayInformation({this.self, this.icon});
}

/// Supplies the HTTP client used for NIP-11 lookups.
///
/// Tests can override this provider to return deterministic relay responses.
final relayInformationHttpClientProvider = Provider<http.Client>((ref) {
  final client = http.Client();
  ref.onDispose(client.close);
  return client;
});

/// Convert a relay websocket or HTTP URL into the NIP-11 HTTP origin.
Uri? relayInformationUri(String relayUrl) {
  try {
    final uri = Uri.parse(relayUrl.trim());
    final scheme = switch (uri.scheme) {
      'wss' => 'https',
      'ws' => 'http',
      'https' || 'http' => uri.scheme,
      _ => null,
    };
    return scheme == null ? null : uri.replace(scheme: scheme);
  } on FormatException {
    return null;
  }
}

/// Normalize NIP-11 `self` to lowercase 64-hex, or null when untrusted.
String? parseRelaySelf(Object? value) {
  if (value is! String) return null;
  final normalized = value.trim().toLowerCase();
  if (!_relayPubkeyPattern.hasMatch(normalized)) return null;
  return normalized;
}

String? _parseIcon(Object? value) {
  if (value is! String) return null;
  final icon = value.trim();
  return icon.isEmpty ? null : icon;
}

/// Fetch and parse a NIP-11 document, failing closed to empty information.
Future<RelayInformation> loadRelayInformation(
  http.Client client,
  String relayUrl,
) async {
  final uri = relayInformationUri(relayUrl);
  if (uri == null) return const RelayInformation();

  try {
    final response = await client
        .get(uri, headers: const {'Accept': 'application/nostr+json'})
        .timeout(const Duration(seconds: 5));
    if (response.statusCode < 200 || response.statusCode >= 300) {
      return const RelayInformation();
    }

    final document = jsonDecode(response.body);
    if (document is! Map<String, dynamic>) {
      return const RelayInformation();
    }
    return RelayInformation(
      self: parseRelaySelf(document['self']),
      icon: _parseIcon(document['icon']),
    );
  } catch (_) {
    return const RelayInformation();
  }
}

/// Reads the public NIP-11 document for [relayUrl].
final relayInformationProvider = FutureProvider.autoDispose
    .family<RelayInformation, String>((ref, relayUrl) async {
      return loadRelayInformation(
        ref.read(relayInformationHttpClientProvider),
        relayUrl,
      );
    });

/// Active community NIP-11 `self` pubkey, or null when untrusted.
final relaySelfProvider = FutureProvider.autoDispose<String?>((ref) async {
  final config = ref.watch(relayConfigProvider);
  final info = await ref.watch(relayInformationProvider(config.baseUrl).future);
  return info.self;
});
