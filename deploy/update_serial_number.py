#!/usr/bin/env python3
# update_serial_number.py
# Populate the `serial_number` column on table `jade_disk` using the `hardware_metadata` column

import json
import re
import pymysql
import argparse

# Database connection settings
DB_CONFIG = {
    "host": "my.database.host",
    "user": "fbloggs",
    "password": "bloggs1234",
    "database": "jade"
}

# Updated regex: Extract everything after the last `_`, allowing `-` in serial numbers
SERIAL_REGEX = re.compile(r'^(ata-|scsi-).+?_([A-Z0-9-]+)$')

def extract_serial(metadata_list):
    """Extracts serial number from metadata list based on known patterns."""
    for entry in metadata_list:
        match = SERIAL_REGEX.match(entry)
        if match:
            return match.group(2), entry  # Extract serial + source entry
    return None, None  # No match found

def update_serial_numbers(dry_run):
    """Fetches hardware_metadata from jade_disk and updates the serial_number field."""
    conn = pymysql.connect(**DB_CONFIG, cursorclass=pymysql.cursors.DictCursor)
    cursor = conn.cursor()

    # Select all rows with non-null hardware_metadata
    cursor.execute("SELECT jade_disk_id, hardware_metadata FROM jade_disk WHERE hardware_metadata IS NOT NULL")
    rows = cursor.fetchall()

    for row in rows:
        disk_id = row['jade_disk_id']
        metadata_json = row['hardware_metadata']

        try:
            metadata_dict = json.loads(metadata_json)
            metadata_list = metadata_dict.get("metadata", [])

            serial_number, source_entry = extract_serial(metadata_list)
            if serial_number:
                if dry_run:
                    print(f"DRY-RUN: Disk ID {disk_id} - Source: {source_entry} -> Extracted Serial: {serial_number}")
                else:
                    print(f"Updating Disk {disk_id} - Extracted Serial: {serial_number}")
                    update_query = "UPDATE jade_disk SET serial_number = %s WHERE jade_disk_id = %s"
                    cursor.execute(update_query, (serial_number, disk_id))

        except json.JSONDecodeError:
            print(f"Skipping disk {disk_id}: Invalid JSON format")

    # Commit updates if not in dry-run mode
    if not dry_run:
        conn.commit()

    cursor.close()
    conn.close()

    print("Serial number update completed." if not dry_run else "Dry-run mode: No changes made.")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Update serial numbers in jade_disk based on hardware_metadata.")
    parser.add_argument("--dry-run", action="store_true", help="Run in dry-run mode without modifying the database")
    args = parser.parse_args()

    update_serial_numbers(args.dry_run)
