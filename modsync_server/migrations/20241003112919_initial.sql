CREATE TABLE modpacks (
    id varchar(128) PRIMARY KEY,
    name varchar(128) UNIQUE NOT NULL,
    game text,
    game_version text,
    modloader text,
    modloader_version text,
    sync_version integer NOT NULL
);

CREATE TABLE files(
    id varchar(128) PRIMARY KEY,
    modpack varchar(128) NOT NULL REFERENCES modpacks(id) ON DELETE CASCADE,
    created_at timestamp with time zone NOT NULL,
    updated_at timestamp with time zone NOT NULL,
    path text NOT NULL,
    state text NOT NULL,
    sync_version integer NOT NULL,
    hash text,
    uploaded boolean NOT NULL
);
CREATE INDEX i_files_modpack ON files (modpack);
CREATE INDEX i_files_modpack_path ON files (path) INCLUDE (modpack);

