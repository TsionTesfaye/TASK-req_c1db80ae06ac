-- 0011_product_imports.sql
-- Product import batches: batch-level tracking + per-row validation state.

CREATE TABLE IF NOT EXISTS import_batches (
    id           UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    uploaded_by  UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    filename     TEXT        NOT NULL,
    kind         TEXT        NOT NULL CHECK (kind IN ('csv','xlsx')),
    status       TEXT        NOT NULL DEFAULT 'uploaded'
        CHECK (status IN ('uploaded','validated','committed','cancelled')),
    row_count    INT         NOT NULL DEFAULT 0,
    error_count  INT         NOT NULL DEFAULT 0,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    committed_at TIMESTAMPTZ,
    cancelled_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS ix_import_batches_uploader ON import_batches (uploaded_by, created_at DESC);
CREATE INDEX IF NOT EXISTS ix_import_batches_status   ON import_batches (status);

CREATE TABLE IF NOT EXISTS import_rows (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    batch_id    UUID        NOT NULL REFERENCES import_batches(id) ON DELETE CASCADE,
    row_number  INT         NOT NULL,
    raw         JSONB       NOT NULL DEFAULT '{}'::JSONB,
    errors      JSONB       NOT NULL DEFAULT '[]'::JSONB,
    valid       BOOLEAN     NOT NULL DEFAULT FALSE,
    UNIQUE (batch_id, row_number)
);
CREATE INDEX IF NOT EXISTS ix_import_rows_batch ON import_rows (batch_id, row_number);
CREATE INDEX IF NOT EXISTS ix_import_rows_invalid ON import_rows (batch_id) WHERE valid = FALSE;
