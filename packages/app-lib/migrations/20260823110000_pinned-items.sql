CREATE TABLE pinned_items (
    kind TEXT NOT NULL,
    ref_id TEXT NOT NULL,
    position INTEGER NOT NULL,
    created INTEGER NOT NULL,
    PRIMARY KEY (kind, ref_id)
);
