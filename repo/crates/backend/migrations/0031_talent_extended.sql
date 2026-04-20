-- 0031_talent_extended.sql
-- Prompt-required candidate + role dimensions that were absent from the
-- original 0030 shape:
--
--   * candidates.major        — field of study (e.g. "Industrial Engineering")
--   * candidates.education    — highest attained level (text so we stay
--                               schema-flexible; handler-side enum normalization)
--   * candidates.availability — start-date or schedule note (e.g.
--                               "2 weeks notice", "immediate", "part-time")
--
--   * roles_open.required_major        — required field of study (optional)
--   * roles_open.min_education         — minimum education level (optional)
--   * roles_open.required_availability — required availability (optional)
--
-- All new columns are nullable so pre-existing rows stay valid. The
-- candidate TSV trigger is updated so search continues to work over the
-- widened profile (major + education surface as searchable text).

ALTER TABLE candidates
    ADD COLUMN IF NOT EXISTS major        TEXT,
    ADD COLUMN IF NOT EXISTS education    TEXT,
    ADD COLUMN IF NOT EXISTS availability TEXT;

ALTER TABLE roles_open
    ADD COLUMN IF NOT EXISTS required_major        TEXT,
    ADD COLUMN IF NOT EXISTS min_education         TEXT,
    ADD COLUMN IF NOT EXISTS required_availability TEXT;

-- Rebuild the TSV trigger to include major + education as searchable text.
CREATE OR REPLACE FUNCTION candidates_refresh_tsv()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    NEW.search_tsv := to_tsvector('english',
        coalesce(NEW.full_name, '') || ' ' ||
        coalesce(array_to_string(NEW.skills, ' '), '') || ' ' ||
        coalesce(NEW.bio, '')      || ' ' ||
        coalesce(NEW.major, '')    || ' ' ||
        coalesce(NEW.education, '')
    );
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS trg_candidates_refresh_tsv ON candidates;
CREATE TRIGGER trg_candidates_refresh_tsv
    BEFORE INSERT OR UPDATE OF full_name, skills, bio, major, education
    ON candidates
    FOR EACH ROW EXECUTE FUNCTION candidates_refresh_tsv();

CREATE INDEX IF NOT EXISTS ix_candidates_major
    ON candidates (major) WHERE major IS NOT NULL AND deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS ix_candidates_availability
    ON candidates (availability) WHERE availability IS NOT NULL AND deleted_at IS NULL;
