ALTER TABLE traces
    ADD COLUMN last_edit_device_id UUID REFERENCES devices(id) ON DELETE SET NULL;
