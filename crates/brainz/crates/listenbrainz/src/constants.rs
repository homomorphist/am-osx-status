//! Constants, mostly taken from <https://listenbrainz.readthedocs.io/en/latest/users/api/core.html#constants>

use core::time::Duration;
use chrono::{DateTime, Utc};

/// The maximum size of a payload in bytes. The same as [`MAX_LISTEN_SIZE`] * [`MAX_LISTENS_PER_REQUEST`].
/// - <https://listenbrainz.readthedocs.io/en/latest/users/api/core.html#listenbrainz.webserver.views.api_tools.MAX_LISTEN_PAYLOAD_SIZE>
pub const MAX_LISTEN_PAYLOAD_SIZE: u64 = 10240000;

/// Maximum overall listen size in bytes, to prevent egregious spamming.
/// - <https://listenbrainz.readthedocs.io/en/latest/users/api/core.html#listenbrainz.webserver.views.api_tools.MAX_LISTEN_PAYLOAD_SIZE>
pub const MAX_LISTEN_SIZE: u64 = 10240000;

/// The max permitted value of duration field.
/// It is currently set to 24 days.
/// - <https://listenbrainz.readthedocs.io/en/latest/users/api/core.html#listenbrainz.webserver.views.api_tools.MAX_DURATION_LIMIT>
pub const MAX_DURATION_LIMIT: Duration = Duration::new(2073600, 0);

/// The maximum number of listens in a request.
/// - <https://listenbrainz.readthedocs.io/en/latest/users/api/core.html#listenbrainz.webserver.views.api_tools.MAX_LISTENS_PER_REQUEST>
pub const MAX_LISTENS_PER_REQUEST: u16 = 1000;

/// The maximum number of listens returned in a single GET request.
/// - <https://listenbrainz.readthedocs.io/en/latest/users/api/core.html#listenbrainz.webserver.views.api_tools.MAX_ITEMS_PER_GET>
pub const MAX_ITEMS_PER_GET: u16 = 1000;

/// The maximum number of tags per listen.
/// - <https://listenbrainz.readthedocs.io/en/latest/users/api/core.html#listenbrainz.webserver.views.api_tools.MAX_TAGS_PER_LISTEN>
pub const MAX_TAGS_PER_LISTEN: u8 = 50;

/// The maximum length of a tag.
/// - <https://listenbrainz.readthedocs.io/en/latest/users/api/core.html#listenbrainz.webserver.views.api_tools.MAX_TAG_SIZE>
pub const MAX_TAG_SIZE: u8 = 64;

/// The earliest acceptable time for a value in the listened_at field.
/// It is currently set to October 1st, 2002.
// TODO: link to field definition in that doc when implement it 
pub const LISTEN_MINIMUM_DATE: DateTime<Utc> = DateTime::from_timestamp(1033430400, 0).unwrap();
