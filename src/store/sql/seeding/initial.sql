-- Insert initial data into session table
INSERT INTO session (ver_crate, ver_music, ver_os, osa_polls_track, osa_polls_music, started_at)
VALUES 
('0.0.0', '1.2.3.4', '14.2.1', 104, 123, (strftime('%s', 'now') - 500) * 1000);

-- Insert initial data into deferred_track table
INSERT INTO deferred_track (title, artist, album, album_artist, album_index, persistent_id, duration, media_kind)
VALUES 
('Snorkel', 'Lumpy', 'Acoustic Hotel', 'Lumpy', 9, 10213095753550683260, 256.240, 'song'),
('Parking La Villette 2', 'Eric La Casa, Jean-Luc Guionnet, Philip Samartzis & Dan Warburton', 'Parking', 'Eric La Casa', 1, 3423445652993440407, 1587.212, 'song');

-- Insert initial data into errors table
INSERT INTO errors (session, fmt_display, fmt_debug)
VALUES 
(1, 'blah blah no wifi', '!!!!! NO WIFI !!!!!'),
(1, 'blah blah no wifi (except again)', '!!!!! NO WIFI (except again) !!!!!');

-- Insert initial data into pending_dispatch table
INSERT INTO pending_dispatch (backend, track, error)
VALUES 
('LastFM', 1, 1),
('LastFM', 2, 2);

