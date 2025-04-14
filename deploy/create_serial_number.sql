-- create_serial_number.sql
-- Create the column `serial_number` in table `jade_disk`

ALTER TABLE jade_disk
ADD COLUMN serial_number VARCHAR(80) NULL AFTER disk_archive_uuid;
