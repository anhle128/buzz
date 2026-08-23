import 'package:buzz/features/channels/channel.dart';
import 'package:buzz/features/channels/channel_management_provider.dart';
import 'package:buzz/features/channels/channel_messages_provider.dart';
import 'package:buzz/features/channels/channel_typing_provider.dart';
import 'package:buzz/features/channels/channels_provider.dart';
import 'package:buzz/shared/mentions/agent_identity_provider.dart';
import 'package:buzz/features/channels/thread_detail_page.dart';
import 'package:buzz/features/channels/thread_replies_provider.dart';
import 'package:buzz/features/channels/timeline_message.dart';
import 'package:buzz/features/profile/profile_provider.dart';
import 'package:buzz/features/profile/user_profile_sheet.dart';
import 'package:buzz/shared/community/relay_information_provider.dart';
import 'package:buzz/shared/profile/user_cache_provider.dart';
import 'package:buzz/shared/profile/user_profile.dart';
import 'package:buzz/shared/relay/app_metadata.dart';
import 'package:buzz/shared/relay/app_metadata_provider.dart';
import 'package:buzz/shared/relay/relay.dart';
import 'package:buzz/shared/theme/theme.dart';
import 'package:buzz/shared/widgets/app_badge.dart';
import 'package:buzz/shared/widgets/avatar_image.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:nostr/nostr.dart' as nostr;
import 'package:shared_preferences/shared_preferences.dart';

const _channelId = 'test-channel';
const _appId = '6eb31227-8ed2-42ec-9024-863497cbeed2';
const _appIdB = 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa';

final _channel = Channel(
  id: _channelId,
  name: 'general',
  channelType: 'stream',
  visibility: 'open',
  description: 'General discussion',
  createdBy: 'abc123',
  createdAt: DateTime(2025),
  memberCount: 5,
  isMember: true,
);

void main() {
  late SharedPreferences prefs;
  late nostr.Keys relay;

  setUp(() async {
    SharedPreferences.setMockInitialValues({});
    prefs = await SharedPreferences.getInstance();
    relay = nostr.Keys.generate();
  });

  NostrEvent appMessage({
    required String content,
    String app = _appId,
    int createdAt = 1000,
    List<List<String>> extraTags = const [],
  }) {
    final event = nostr.Event.from(
      kind: EventKind.streamMessage,
      content: content,
      secretKey: relay.secret,
      createdAt: createdAt,
      tags: [
        ['h', _channelId],
        ['buzz:app', app],
        ...extraTags,
      ],
      verify: true,
    );
    return NostrEvent.fromJson(event.toMap());
  }

  Map<String, AppMetadata> apps({
    String? picture = 'https://example.test/archon.png',
  }) => {
    _appId: AppMetadata(
      appId: _appId,
      name: 'Archon',
      picture: picture,
      status: 'active',
      eventId: 'ab' * 32,
      relayPubkey: relay.public.toLowerCase(),
      updatedAt: 1700000000,
    ),
    _appIdB: AppMetadata(
      appId: _appIdB,
      name: 'PagerDuty',
      status: 'active',
      eventId: 'cd' * 32,
      relayPubkey: relay.public.toLowerCase(),
      updatedAt: 1700000000,
    ),
  };

  List<TimelineMessage> timeline(List<NostrEvent> events) {
    return formatTimeline(events, relaySelfPubkey: relay.public, apps: apps());
  }

  Widget buildThread({
    required TimelineMessage head,
    required List<TimelineMessage> messages,
    List<NostrEvent> liveEvents = const [],
    UserProfile? currentUser,
    Map<String, UserProfile> users = const {},
  }) {
    return ProviderScope(
      overrides: [
        savedPrefsProvider.overrideWithValue(prefs),
        appMetadataProvider.overrideWith((ref) async => apps()),
        relaySelfProvider.overrideWith((ref) async => relay.public),
        channelMessagesProvider(
          _channelId,
        ).overrideWith(() => _FakeMessagesNotifier(liveEvents)),
        channelTypingProvider(
          _channelId,
        ).overrideWith(() => _FakeTypingNotifier()),
        threadRepliesProvider(
          ThreadRepliesArgs(channelId: _channelId, rootId: head.id),
        ).overrideWith((ref) async => liveEvents),
        userCacheProvider.overrideWith(() => _FakeUserCacheNotifier(users)),
        profileProvider.overrideWith(() => _FakeProfileNotifier(currentUser)),
        channelsProvider.overrideWith(() => _FakeChannelsNotifier([_channel])),
        channelDetailsProvider(
          _channelId,
        ).overrideWith((ref) async => ChannelDetails.fromChannel(_channel)),
        channelMembersProvider(
          _channelId,
        ).overrideWith((ref) async => const []),
        channelBotPubkeysProvider(
          _channelId,
        ).overrideWith((ref) async => const <String>{}),
        agentOwnersProvider.overrideWith(
          (ref) async => const <String, String>{},
        ),
        relayClientProvider.overrideWithValue(
          RelayClient(baseUrl: 'http://localhost:3000'),
        ),
      ],
      child: MaterialApp(
        theme: AppTheme.light(),
        home: ThreadDetailPage(
          threadHead: head,
          allMessages: messages,
          channelId: _channelId,
          currentPubkey: currentUser?.pubkey ?? 'self',
          isMember: true,
          isArchived: false,
        ),
      ),
    );
  }

  testWidgets('thread head renders App name, badge, and static avatar', (
    tester,
  ) async {
    final root = appMessage(content: 'callback');
    final messages = timeline([root]);
    await tester.pumpWidget(
      buildThread(head: messages.single, messages: messages),
    );
    await tester.pumpAndSettle();

    expect(find.text('Archon'), findsOneWidget);
    expect(find.byType(AppBadge), findsOneWidget);
    expect(find.text('App'), findsOneWidget);
    expect(
      tester.widget<AvatarImage>(find.byType(AvatarImage).first).imageUrl,
      'https://example.test/archon.png',
    );

    await tester.tap(find.text('Archon'));
    await tester.pumpAndSettle();
    expect(find.byType(UserProfileSheet), findsNothing);
  });

  testWidgets('thread App without picture ignores the relay profile avatar', (
    tester,
  ) async {
    final root = appMessage(content: 'callback');
    final messages = formatTimeline(
      [root],
      relaySelfPubkey: relay.public,
      apps: apps(picture: null),
    );
    await tester.pumpWidget(
      buildThread(
        head: messages.single,
        messages: messages,
        users: {
          relay.public.toLowerCase(): UserProfile(
            pubkey: relay.public,
            displayName: 'Relay Bot',
            avatarUrl: 'https://example.test/relay.png',
          ),
        },
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('Archon'), findsOneWidget);
    expect(find.text('A'), findsOneWidget);
    expect(
      tester.widget<AvatarImage>(find.byType(AvatarImage).first).imageUrl,
      isNull,
    );
  });

  testWidgets(
    'thread replies from two Apps stay ungrouped and have no user sheet',
    (tester) async {
      final root = appMessage(content: 'root callback');
      // Fill the reply's parent after we know the root id.
      final replyEvent = nostr.Event.from(
        kind: EventKind.streamMessage,
        content: 'pager reply',
        secretKey: relay.secret,
        createdAt: 1060,
        tags: [
          ['h', _channelId],
          ['buzz:app', _appIdB],
          ['e', root.id, '', 'reply'],
        ],
        verify: true,
      );
      final signedReply = NostrEvent.fromJson(replyEvent.toMap());
      final messages = timeline([root, signedReply]);
      await tester.pumpWidget(
        buildThread(
          head: messages.first,
          messages: messages,
          liveEvents: [root, signedReply],
        ),
      );
      await tester.pumpAndSettle();

      expect(find.text('Archon'), findsOneWidget);
      expect(find.text('PagerDuty'), findsOneWidget);
      expect(find.byType(AppBadge), findsNWidgets(2));

      await tester.tap(find.text('PagerDuty'));
      await tester.pumpAndSettle();
      expect(find.byType(UserProfileSheet), findsNothing);
    },
  );

  testWidgets('thread App messages hide user-only management actions', (
    tester,
  ) async {
    final root = appMessage(content: 'callback');
    final messages = timeline([root]);
    await tester.pumpWidget(
      buildThread(
        head: messages.single,
        messages: messages,
        currentUser: UserProfile(pubkey: relay.public, displayName: 'Relay'),
      ),
    );
    await tester.pumpAndSettle();

    await tester.longPress(
      find.byKey(ValueKey('thread-message-row-${root.id}')),
    );
    await tester.pumpAndSettle();

    expect(find.text('Edit message'), findsNothing);
    expect(find.text('Delete message'), findsNothing);
  });
}

class _FakeMessagesNotifier extends ChannelMessagesNotifier {
  _FakeMessagesNotifier(this._messages) : super(_channelId);
  final List<NostrEvent> _messages;

  @override
  AsyncValue<List<NostrEvent>> build() => AsyncData(_messages);

  @override
  bool get hasLoadedMessages => true;
}

class _FakeTypingNotifier extends ChannelTypingNotifier {
  _FakeTypingNotifier() : super(_channelId);

  @override
  List<TypingEntry> build() => const [];
}

class _FakeUserCacheNotifier extends UserCacheNotifier {
  _FakeUserCacheNotifier(this._users);
  final Map<String, UserProfile> _users;

  @override
  Map<String, UserProfile> build() => _users;

  @override
  UserProfile? get(String pubkey) => _users[pubkey.toLowerCase()];
}

class _FakeProfileNotifier extends ProfileNotifier {
  _FakeProfileNotifier([this.profile]);
  final UserProfile? profile;

  @override
  Future<UserProfile?> build() async =>
      profile ?? const UserProfile(pubkey: 'self', displayName: 'Self');
}

class _FakeChannelsNotifier extends ChannelsNotifier {
  _FakeChannelsNotifier(this._channels);
  final List<Channel> _channels;

  @override
  Future<List<Channel>> build() => SynchronousFuture(_channels);
}
