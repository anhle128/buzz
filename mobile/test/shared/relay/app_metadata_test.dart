import 'package:buzz/shared/relay/app_metadata.dart';
import 'package:buzz/shared/relay/nostr_models.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nostr/nostr.dart' as nostr;

final _relay = nostr.Keys.generate();
final _other = nostr.Keys.generate();

const _appId = '6eb31227-8ed2-42ec-9024-863497cbeed2';
const _otherAppId = 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa';

NostrEvent _fromNostr(nostr.Event event) => NostrEvent.fromJson(event.toMap());

String _flipHexNibble(String hex) {
  final last = hex[hex.length - 1];
  return '${hex.substring(0, hex.length - 1)}${last == '0' ? '1' : '0'}';
}

nostr.Event _signMetadata({
  nostr.Keys? keys,
  int kind = EventKind.appMetadata,
  String appId = _appId,
  String name = 'Buildkite',
  String status = 'active',
  String description = 'Build notifications',
  String? picture,
  int createdAt = 1700000000,
  List<List<String>> extraTags = const [],
  List<List<String>>? tags,
}) {
  return nostr.Event.from(
    kind: kind,
    content: description,
    secretKey: (keys ?? _relay).secret,
    createdAt: createdAt,
    tags:
        tags ??
        [
          ['d', appId],
          ['name', name],
          ['status', status],
          if (picture != null) ['picture', picture],
          ...extraTags,
        ],
    verify: true,
  );
}

nostr.Event _signMessage({
  nostr.Keys? keys,
  String appId = _appId,
  List<List<String>> extraTags = const [],
}) {
  return nostr.Event.from(
    kind: EventKind.streamMessage,
    content: 'build passed',
    secretKey: (keys ?? _relay).secret,
    createdAt: 1700000100,
    tags: [
      ['h', '36411e44-0e2d-4cfe-bd6e-567eb169db9f'],
      ['buzz:app', appId],
      ...extraTags,
    ],
    verify: true,
  );
}

void main() {
  test('parses valid active metadata', () {
    final event = _signMetadata();
    final parsed = parseAppMetadata(_fromNostr(event), _relay.public);
    expect(parsed, isNotNull);
    expect(parsed!.appId, _appId);
    expect(parsed.name, 'Buildkite');
    expect(parsed.description, 'Build notifications');
    expect(parsed.status, 'active');
    expect(parsed.eventId, event.id);
    expect(parsed.relayPubkey, _relay.public.toLowerCase());
    expect(parsed.updatedAt, 1700000000);
    expect(parsed.picture, isNull);
  });

  test('parses valid disabled metadata with picture', () {
    final event = _signMetadata(
      status: 'disabled',
      picture: 'https://example.test/icon.png',
      description: '',
    );
    final parsed = parseAppMetadata(_fromNostr(event), _relay.public);
    expect(parsed, isNotNull);
    expect(parsed!.appId, _appId);
    expect(parsed.name, 'Buildkite');
    expect(parsed.picture, 'https://example.test/icon.png');
    expect(parsed.status, 'disabled');
    expect(parsed.eventId, event.id);
    expect(parsed.relayPubkey, _relay.public.toLowerCase());
    expect(parsed.updatedAt, 1700000000);
    expect(parsed.description, isNull);
  });

  test('rejects invalid event ID', () {
    final signed = _fromNostr(_signMetadata());
    final event = NostrEvent(
      id: '0' * 64,
      pubkey: signed.pubkey,
      createdAt: signed.createdAt,
      kind: signed.kind,
      tags: signed.tags,
      content: signed.content,
      sig: signed.sig,
    );
    expect(parseAppMetadata(event, _relay.public), isNull);
  });

  test('rejects invalid signature', () {
    final signed = _fromNostr(_signMetadata());
    final event = NostrEvent(
      id: signed.id,
      pubkey: signed.pubkey,
      createdAt: signed.createdAt,
      kind: signed.kind,
      tags: signed.tags,
      content: signed.content,
      sig: _flipHexNibble(signed.sig),
    );
    expect(parseAppMetadata(event, _relay.public), isNull);
  });

  test('rejects the wrong relay author', () {
    expect(
      parseAppMetadata(_fromNostr(_signMetadata(keys: _other)), _relay.public),
      isNull,
    );
    expect(
      parseAppMetadata(_fromNostr(_signMetadata()), _other.public),
      isNull,
    );
  });

  test('rejects malformed or duplicate tags', () {
    expect(
      parseAppMetadata(
        _fromNostr(
          _signMetadata(
            extraTags: const [
              ['name', 'Pager'],
            ],
          ),
        ),
        _relay.public,
      ),
      isNull,
    );
    expect(
      parseAppMetadata(
        _fromNostr(
          _signMetadata(
            extraTags: const [
              ['d', _otherAppId],
            ],
          ),
        ),
        _relay.public,
      ),
      isNull,
    );
    expect(
      parseAppMetadata(
        _fromNostr(
          _signMetadata(
            tags: const [
              ['d', _appId],
              ['name'],
              ['status', 'active'],
            ],
          ),
        ),
        _relay.public,
      ),
      isNull,
    );
    expect(
      parseAppMetadata(
        _fromNostr(
          _signMetadata(
            extraTags: const [
              ['picture', 'https://a.test/x.png'],
            ],
            picture: 'https://b.test/y.png',
          ),
        ),
        _relay.public,
      ),
      isNull,
    );
    expect(
      parseAppMetadata(_fromNostr(_signMetadata(name: '')), _relay.public),
      isNull,
    );
    expect(
      parseAppMetadata(
        _fromNostr(_signMetadata(status: 'archived')),
        _relay.public,
      ),
      isNull,
    );
  });

  test('rejects invalid UUID d tags', () {
    expect(
      parseAppMetadata(
        _fromNostr(_signMetadata(appId: _appId.toUpperCase())),
        _relay.public,
      ),
      isNull,
    );
    expect(
      parseAppMetadata(
        _fromNostr(_signMetadata(appId: '6eb312278ed242ec9024863497cbeed2')),
        _relay.public,
      ),
      isNull,
    );
    expect(
      parseAppMetadata(
        _fromNostr(_signMetadata(appId: 'not-a-uuid')),
        _relay.public,
      ),
      isNull,
    );
  });

  test('folds the latest valid head per App UUID', () {
    final older = _signMetadata(name: 'Old', createdAt: 100, description: 'v1');
    final newer = _signMetadata(
      name: 'New',
      status: 'disabled',
      createdAt: 200,
      description: 'v2',
      picture: 'https://example.test/icon.png',
    );
    final other = _signMetadata(
      appId: _otherAppId,
      name: 'Other',
      createdAt: 150,
      description: '',
    );
    final wrongRelay = _signMetadata(
      keys: _other,
      name: 'Spoof',
      createdAt: 300,
    );
    final folded = foldAppMetadataHeads([
      _fromNostr(older),
      _fromNostr(newer),
      _fromNostr(other),
      _fromNostr(wrongRelay),
    ], _relay.public);
    expect(folded.length, 2);
    expect(folded[_appId]?.name, 'New');
    expect(folded[_appId]?.description, 'v2');
    expect(folded[_appId]?.picture, 'https://example.test/icon.png');
    expect(folded[_appId]?.status, 'disabled');
    expect(folded[_appId]?.eventId, newer.id);
    expect(folded[_appId]?.updatedAt, 200);
    expect(folded[_otherAppId]?.name, 'Other');
  });

  test('tie-breaks equal created_at by lower event id', () {
    final left = _signMetadata(name: 'Left', createdAt: 50);
    final right = _signMetadata(name: 'Right', createdAt: 50);
    final winner = left.id.compareTo(right.id) < 0 ? 'Left' : 'Right';
    final folded = foldAppMetadataHeads([
      _fromNostr(left),
      _fromNostr(right),
    ], _relay.public);
    expect(folded.length, 1);
    expect(folded[_appId]?.name, winner);
  });

  test('resolves only metadata whose App UUID and relay match the message', () {
    final event = _fromNostr(_signMessage());
    final valid = AppMetadata(
      appId: _appId,
      name: 'Buildkite',
      status: 'active',
      eventId: 'ab' * 32,
      relayPubkey: _relay.public.toLowerCase(),
      updatedAt: 1700000000,
    );

    expect(
      resolveAppActor(
        event: event,
        apps: {_appId: valid},
        relaySelfPubkey: _relay.public,
      )?.appId,
      _appId,
    );
    expect(
      resolveAppActor(
        event: event,
        apps: {
          _appId: AppMetadata(
            appId: _otherAppId,
            name: valid.name,
            status: valid.status,
            eventId: valid.eventId,
            relayPubkey: valid.relayPubkey,
            updatedAt: valid.updatedAt,
          ),
        },
        relaySelfPubkey: _relay.public,
      ),
      isNull,
    );
    expect(
      resolveAppActor(
        event: event,
        apps: {
          _appId: AppMetadata(
            appId: valid.appId,
            name: valid.name,
            status: valid.status,
            eventId: valid.eventId,
            relayPubkey: _other.public.toLowerCase(),
            updatedAt: valid.updatedAt,
          ),
        },
        relaySelfPubkey: _relay.public,
      ),
      isNull,
    );
  });
}
