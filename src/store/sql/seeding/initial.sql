INSERT INTO sessions (ver_crate, ver_player, ver_os, osa_fetches_track, osa_fetches_player, started_at) VALUES 
    ('0.0.0', '1.2.3.4', '14.2.1', 104, 123, (strftime('%s', 'now') - 500) * 1000);

INSERT INTO deferred_tracks (title, artist, album, album_artist, album_index, persistent_id, duration, media_kind) VALUES 
    ('Snorkel'              , 'Lumpy'                                                            , 'Acoustic Hotel', 'Lumpy'       , 9, -8233648320158868356, 256.240,  'song'),
    ('Parking La Villette 2', 'Eric La Casa, Jean-Luc Guionnet, Philip Samartzis & Dan Warburton', 'Parking'       , 'Eric La Casa', 1,  3423445652993440407, 1587.212, 'song');

INSERT INTO errors (session, fmt_display, fmt_debug) VALUES 
    (1, 'blah blah no wifi'               , '!!!!! NO WIFI !!!!!'),
    (1, 'blah blah no wifi (except again)', '!!!!! NO WIFI (except again) !!!!!');

INSERT INTO pending_dispatches (backend, track, error) VALUES 
    ('LastFM', 1, 1),
    ('LastFM', 2, 2);

