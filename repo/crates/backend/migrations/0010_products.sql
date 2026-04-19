-- 0010_products.sql
-- Products catalog: products, tax rates, images, and immutable change history.

CREATE TABLE IF NOT EXISTS products (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    sku           TEXT        NOT NULL UNIQUE,
    name          TEXT        NOT NULL,
    description   TEXT,
    category_id   UUID        REFERENCES categories(id) ON DELETE SET NULL,
    brand_id      UUID        REFERENCES brands(id) ON DELETE SET NULL,
    unit_id       UUID        REFERENCES units(id) ON DELETE SET NULL,
    site_id       UUID        REFERENCES sites(id) ON DELETE SET NULL,
    department_id UUID        REFERENCES departments(id) ON DELETE SET NULL,
    on_shelf      BOOLEAN     NOT NULL DEFAULT TRUE,
    price_cents   INT         NOT NULL DEFAULT 0 CHECK (price_cents >= 0),
    currency      TEXT        NOT NULL DEFAULT 'USD',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at    TIMESTAMPTZ,
    created_by    UUID        REFERENCES users(id) ON DELETE SET NULL,
    updated_by    UUID        REFERENCES users(id) ON DELETE SET NULL
);
CREATE INDEX IF NOT EXISTS ix_products_sku       ON products (sku);
CREATE INDEX IF NOT EXISTS ix_products_site      ON products (site_id);
CREATE INDEX IF NOT EXISTS ix_products_dept      ON products (department_id);
CREATE INDEX IF NOT EXISTS ix_products_category  ON products (category_id);
CREATE INDEX IF NOT EXISTS ix_products_brand     ON products (brand_id);
CREATE INDEX IF NOT EXISTS ix_products_on_shelf  ON products (on_shelf) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS ix_products_deleted   ON products (deleted_at) WHERE deleted_at IS NOT NULL;

CREATE TABLE IF NOT EXISTS product_tax_rates (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    product_id  UUID        NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    state_code  TEXT        NOT NULL REFERENCES state_codes(code),
    rate_bp     INT         NOT NULL CHECK (rate_bp >= 0),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (product_id, state_code)
);
CREATE INDEX IF NOT EXISTS ix_product_tax_rates_product ON product_tax_rates (product_id);

CREATE TABLE IF NOT EXISTS product_images (
    id           UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    product_id   UUID        NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    storage_path TEXT        NOT NULL,
    content_type TEXT        NOT NULL DEFAULT 'application/octet-stream',
    size_bytes   INT         NOT NULL DEFAULT 0,
    uploaded_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    uploaded_by  UUID        REFERENCES users(id) ON DELETE SET NULL
);
CREATE INDEX IF NOT EXISTS ix_product_images_product ON product_images (product_id);

CREATE TABLE IF NOT EXISTS product_history (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    product_id  UUID        NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    action      TEXT        NOT NULL
        CHECK (action IN ('create','update','delete','status','tax_rate','image')),
    changed_by  UUID        REFERENCES users(id) ON DELETE SET NULL,
    changed_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    before_json JSONB,
    after_json  JSONB
);
CREATE INDEX IF NOT EXISTS ix_product_history_product ON product_history (product_id, changed_at DESC);

-- Immutability trigger: forbid UPDATE or DELETE on product_history rows.
CREATE OR REPLACE FUNCTION product_history_immutable()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'product_history rows are immutable';
END;
$$;

DROP TRIGGER IF EXISTS trg_product_history_immutable ON product_history;
CREATE TRIGGER trg_product_history_immutable
    BEFORE UPDATE OR DELETE ON product_history
    FOR EACH ROW EXECUTE FUNCTION product_history_immutable();
