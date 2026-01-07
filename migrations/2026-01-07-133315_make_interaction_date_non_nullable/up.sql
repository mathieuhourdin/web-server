UPDATE interactions 
SET interaction_date = COALESCE(interaction_date, created_at)
WHERE interaction_date IS NULL;

-- Rendre la colonne NOT NULL avec une valeur par défaut
ALTER TABLE interactions 
ALTER COLUMN interaction_date SET DEFAULT CURRENT_TIMESTAMP,
ALTER COLUMN interaction_date SET NOT NULL;