CREATE TABLE hosted_servers (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    icon_path TEXT NULL,
    game_version TEXT NOT NULL,
    loader TEXT NOT NULL,
    loader_version TEXT NULL,
    provider TEXT NOT NULL,
    project_id TEXT NULL,
    version_id TEXT NULL,
    port INTEGER NOT NULL DEFAULT 25565,
    max_memory_mb INTEGER NOT NULL DEFAULT 4096,
    extra_java_args TEXT NULL,
    eula_accepted BOOLEAN NOT NULL DEFAULT FALSE,
    install_stage TEXT NOT NULL DEFAULT 'not_installed',
    created INTEGER NOT NULL,
    modified INTEGER NOT NULL
);
