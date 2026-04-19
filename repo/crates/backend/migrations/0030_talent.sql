-- 0030_talent.sql
-- Talent Intelligence module: candidates, open roles, weights, watchlists, feedback.

-- ── Candidates ──────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS candidates (
    id                  UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    full_name           TEXT        NOT NULL,
    email_mask          TEXT        NOT NULL,
    location            TEXT,
    years_experience    INT         NOT NULL DEFAULT 0,
    skills              TEXT[]      NOT NULL DEFAULT '{}',
    bio                 TEXT,
    completeness_score  INT         NOT NULL DEFAULT 0 CHECK (completeness_score BETWEEN 0 AND 100),
    last_active_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at          TIMESTAMPTZ
);

-- Full-text search vector: name + skills + bio.
-- Populated via trigger because `to_tsvector(regconfig, text)` is STABLE,
-- not IMMUTABLE, so it cannot be used in a GENERATED STORED column.
ALTER TABLE candidates
    ADD COLUMN IF NOT EXISTS search_tsv tsvector;

CREATE OR REPLACE FUNCTION candidates_refresh_tsv()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    NEW.search_tsv := to_tsvector('english',
        coalesce(NEW.full_name, '') || ' ' ||
        coalesce(array_to_string(NEW.skills, ' '), '') || ' ' ||
        coalesce(NEW.bio, '')
    );
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS trg_candidates_refresh_tsv ON candidates;
CREATE TRIGGER trg_candidates_refresh_tsv
    BEFORE INSERT OR UPDATE OF full_name, skills, bio ON candidates
    FOR EACH ROW EXECUTE FUNCTION candidates_refresh_tsv();

CREATE INDEX IF NOT EXISTS ix_candidates_tsv
    ON candidates USING GIN (search_tsv);

CREATE INDEX IF NOT EXISTS ix_candidates_active
    ON candidates (last_active_at DESC)
    WHERE deleted_at IS NULL;

-- ── Open Roles ───────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS roles_open (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    title           TEXT        NOT NULL,
    department_id   UUID        REFERENCES departments(id) ON DELETE SET NULL,
    required_skills TEXT[]      NOT NULL DEFAULT '{}',
    min_years       INT         NOT NULL DEFAULT 0,
    site_id         UUID        REFERENCES sites(id) ON DELETE SET NULL,
    status          TEXT        NOT NULL DEFAULT 'open'
                                CHECK (status IN ('open', 'closed', 'filled')),
    opened_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by      UUID        REFERENCES users(id) ON DELETE SET NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS ix_roles_open_status ON roles_open (status);

-- ── Talent Weights (per user, self-scoped) ───────────────────────────────────

CREATE TABLE IF NOT EXISTS talent_weights (
    user_id             UUID    PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    skills_weight       INT     NOT NULL DEFAULT 40
                                CHECK (skills_weight >= 0),
    experience_weight   INT     NOT NULL DEFAULT 30
                                CHECK (experience_weight >= 0),
    recency_weight      INT     NOT NULL DEFAULT 15
                                CHECK (recency_weight >= 0),
    completeness_weight INT     NOT NULL DEFAULT 15
                                CHECK (completeness_weight >= 0),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_weights_sum_100
        CHECK (skills_weight + experience_weight + recency_weight + completeness_weight = 100)
);

-- ── Watchlists (self-scoped) ─────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS talent_watchlists (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_id    UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name        TEXT        NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS ix_watchlists_owner ON talent_watchlists (owner_id);

CREATE TABLE IF NOT EXISTS talent_watchlist_items (
    watchlist_id    UUID        NOT NULL REFERENCES talent_watchlists(id) ON DELETE CASCADE,
    candidate_id    UUID        NOT NULL REFERENCES candidates(id) ON DELETE CASCADE,
    added_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (watchlist_id, candidate_id)
);

-- ── Feedback (PERM talent.feedback, owner-scoped) ────────────────────────────

CREATE TABLE IF NOT EXISTS talent_feedback (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    candidate_id    UUID        NOT NULL REFERENCES candidates(id) ON DELETE CASCADE,
    role_id         UUID        REFERENCES roles_open(id) ON DELETE SET NULL,
    owner_id        UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    thumb           TEXT        NOT NULL CHECK (thumb IN ('up', 'down')),
    note            TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS ix_feedback_candidate ON talent_feedback (candidate_id);
CREATE INDEX IF NOT EXISTS ix_feedback_owner ON talent_feedback (owner_id);
