-- 0012_product_extended.sql
-- Prompt-required product master-data dimensions that were absent from the
-- original 0010 shape: SPU (standard product unit — a grouping key that
-- buckets multiple SKUs together), barcode (GTIN/UPC/EAN surface scanned
-- at register), and shelf_life_days (operational freshness window).
--
-- All three are optional (NULL allowed) so existing seeded and imported
-- rows stay valid; `barcode` is UNIQUE where present so scan lookups are
-- deterministic.

ALTER TABLE products
    ADD COLUMN IF NOT EXISTS spu             TEXT,
    ADD COLUMN IF NOT EXISTS barcode         TEXT,
    ADD COLUMN IF NOT EXISTS shelf_life_days INT CHECK (shelf_life_days IS NULL OR shelf_life_days >= 0);

CREATE UNIQUE INDEX IF NOT EXISTS ix_products_barcode_unique
    ON products (barcode)
    WHERE barcode IS NOT NULL AND deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS ix_products_spu
    ON products (spu)
    WHERE spu IS NOT NULL AND deleted_at IS NULL;
