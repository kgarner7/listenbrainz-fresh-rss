-- Your SQL goes here
CREATE TABLE releases (
  id TEXT PRIMARY KEY NOT NULL,
  has_front BOOLEAN NOT NULL,
  urls TEXT NOT NULL
)