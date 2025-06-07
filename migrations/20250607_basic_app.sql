CREATE TABLE posts(id TEXT, public_key TEXT, timestamp INTEGER, body TEXT);
CREATE TABLE follows(public_key TEXT, target TEXT, state BOOLEAN, sequence INTEGER);