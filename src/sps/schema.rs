// schema.rs

// @generated automatically by Diesel CLI.

diesel::table! {
    jade_bundle (jade_bundle_id) {
        jade_bundle_id -> Bigint,
        #[max_length = 255]
        bundle_file -> Nullable<Varchar>,
        capacity -> Nullable<Bigint>,
        closed -> Nullable<Bit>,
        date_created -> Nullable<Datetime>,
        date_updated -> Nullable<Datetime>,
        version -> Nullable<Bigint>,
        jade_host_id -> Nullable<Bigint>,
        #[max_length = 255]
        checksum -> Nullable<Varchar>,
        size -> Nullable<Bigint>,
        #[max_length = 255]
        jade_bundle_uuid -> Nullable<Varchar>,
        #[max_length = 255]
        tdrss_archive_uuid -> Nullable<Varchar>,
        date_last_uploaded -> Nullable<Datetime>,
    }
}

diesel::table! {
    jade_contact (jade_contact_id) {
        jade_contact_id -> Bigint,
        date_created -> Nullable<Datetime>,
        date_updated -> Nullable<Datetime>,
        #[max_length = 255]
        email -> Nullable<Varchar>,
        #[max_length = 255]
        contact_name -> Nullable<Varchar>,
        #[max_length = 255]
        contact_role -> Nullable<Varchar>,
        version -> Nullable<Bigint>,
    }
}

diesel::table! {
    jade_credentials (jade_credentials_id) {
        jade_credentials_id -> Bigint,
        date_created -> Nullable<Datetime>,
        date_updated -> Nullable<Datetime>,
        #[max_length = 255]
        password -> Nullable<Varchar>,
        #[max_length = 255]
        ssh_key_path -> Nullable<Varchar>,
        #[max_length = 255]
        username -> Nullable<Varchar>,
        version -> Nullable<Bigint>,
    }
}

diesel::table! {
    jade_data_stream (jade_data_stream_id) {
        jade_data_stream_id -> Bigint,
        active -> Nullable<Bit>,
        #[max_length = 255]
        binary_suffix -> Nullable<Varchar>,
        calculate_ingest_checksum -> Nullable<Bit>,
        #[max_length = 255]
        compression -> Nullable<Varchar>,
        date_created -> Nullable<Datetime>,
        date_updated -> Nullable<Datetime>,
        #[max_length = 255]
        file_host -> Nullable<Varchar>,
        #[max_length = 255]
        file_path -> Nullable<Varchar>,
        #[max_length = 255]
        file_prefix -> Nullable<Varchar>,
        repeat_seconds -> Nullable<Integer>,
        satellite -> Nullable<Bit>,
        #[max_length = 255]
        semaphore_suffix -> Nullable<Varchar>,
        verify_remote_checksum -> Nullable<Bit>,
        verify_remote_length -> Nullable<Bit>,
        version -> Nullable<Bigint>,
        #[max_length = 255]
        workflow_bean -> Nullable<Varchar>,
        xfer_limit_kbits_sec -> Nullable<Bigint>,
        jade_credentials_id -> Nullable<Bigint>,
        jade_stream_metadata_id -> Nullable<Bigint>,
    }
}

diesel::table! {
    jade_data_stream_metrics (jade_data_stream_metrics_id) {
        jade_data_stream_metrics_id -> Bigint,
        date_created -> Nullable<Datetime>,
        date_updated -> Nullable<Datetime>,
        file_count -> Nullable<Bigint>,
        file_pair_count -> Nullable<Bigint>,
        file_pair_size -> Nullable<Bigint>,
        file_size -> Nullable<Bigint>,
        date_oldest_file -> Nullable<Datetime>,
        version -> Nullable<Bigint>,
        jade_data_stream_id -> Nullable<Bigint>,
    }
}

diesel::table! {
    jade_data_streams_json (jade_data_streams_json_id) {
        jade_data_streams_json_id -> Bigint,
        id -> Bigint,
        #[max_length = 36]
        uuid -> Char,
        active -> Bit,
        archive_disk_ara -> Bit,
        archive_disk_icecube -> Bit,
        archive_satellite_ara -> Bit,
        archive_satellite_icecube -> Bit,
        #[max_length = 32]
        retro_disk_policy -> Varchar,
        #[max_length = 255]
        stream_desc -> Varchar,
    }
}

diesel::table! {
    jade_disk (jade_disk_id) {
        jade_disk_id -> Bigint,
        bad -> Nullable<Bit>,
        capacity -> Nullable<Bigint>,
        closed -> Nullable<Bit>,
        copy_id -> Nullable<Integer>,
        date_created -> Nullable<Datetime>,
        date_updated -> Nullable<Datetime>,
        #[max_length = 255]
        device_path -> Nullable<Varchar>,
        #[max_length = 255]
        label -> Nullable<Varchar>,
        on_hold -> Nullable<Bit>,
        #[max_length = 36]
        uuid -> Nullable<Char>,
        version -> Nullable<Bigint>,
        jade_host_id -> Nullable<Bigint>,
        #[max_length = 255]
        disk_archive_uuid -> Nullable<Varchar>,
        hardware_metadata -> Nullable<Text>,
    }
}

diesel::table! {
    jade_disk_archival_record (jade_disk_archival_record_id) {
        jade_disk_archival_record_id -> Bigint,
        date_created -> Nullable<Datetime>,
        date_updated -> Nullable<Datetime>,
        version -> Nullable<Bigint>,
        jade_disk_id -> Nullable<Bigint>,
        jade_file_pair_id -> Nullable<Bigint>,
        jade_host_id -> Nullable<Bigint>,
    }
}

diesel::table! {
    jade_disk_archive (jade_disk_archive_id) {
        jade_disk_archive_id -> Bigint,
        capacity -> Nullable<Bigint>,
        date_created -> Nullable<Datetime>,
        date_updated -> Nullable<Datetime>,
        #[max_length = 255]
        description -> Nullable<Varchar>,
        number_of_copies -> Nullable<Integer>,
        version -> Nullable<Bigint>,
    }
}

diesel::table! {
    jade_disk_label (jade_disk_label_id) {
        jade_disk_label_id -> Bigint,
        version -> Nullable<Bigint>,
        date_created -> Nullable<Datetime>,
        date_updated -> Nullable<Datetime>,
        #[max_length = 36]
        disk_archive_uuid -> Nullable<Char>,
        copy_id -> Nullable<Integer>,
        disk_archive_year -> Nullable<Integer>,
        disk_archive_sequence -> Nullable<Integer>,
    }
}

diesel::table! {
    jade_file_pair (jade_file_pair_id) {
        jade_file_pair_id -> Bigint,
        #[max_length = 255]
        archive_checksum -> Nullable<Varchar>,
        #[max_length = 255]
        archive_file -> Nullable<Varchar>,
        archive_size -> Nullable<Bigint>,
        #[max_length = 255]
        binary_file -> Nullable<Varchar>,
        binary_size -> Nullable<Bigint>,
        date_archived -> Nullable<Datetime>,
        date_created -> Nullable<Datetime>,
        date_fetched -> Nullable<Datetime>,
        date_processed -> Nullable<Datetime>,
        date_updated -> Nullable<Datetime>,
        date_verified -> Nullable<Datetime>,
        #[max_length = 255]
        fetch_checksum -> Nullable<Varchar>,
        #[max_length = 255]
        fingerprint -> Nullable<Varchar>,
        ingest_checksum -> Nullable<Bigint>,
        #[max_length = 255]
        metadata_file -> Nullable<Varchar>,
        #[max_length = 255]
        origin_checksum -> Nullable<Varchar>,
        date_modified_origin -> Nullable<Datetime>,
        #[max_length = 255]
        semaphore_file -> Nullable<Varchar>,
        version -> Nullable<Bigint>,
        archived_by_host_id -> Nullable<Bigint>,
        jade_data_stream_id -> Nullable<Bigint>,
        fetched_by_host_id -> Nullable<Bigint>,
        processed_by_host_id -> Nullable<Bigint>,
        verified_by_host_id -> Nullable<Bigint>,
        #[max_length = 255]
        jade_data_stream_uuid -> Nullable<Varchar>,
        #[max_length = 255]
        jade_file_pair_uuid -> Nullable<Varchar>,
        #[max_length = 255]
        priority_group -> Nullable<Varchar>,
        #[max_length = 255]
        data_warehouse_path -> Nullable<Varchar>,
    }
}

diesel::table! {
    jade_file_pair_metadata (jade_file_pair_metadata_id) {
        jade_file_pair_metadata_id -> Bigint,
        date_created -> Nullable<Datetime>,
        date_updated -> Nullable<Datetime>,
        version -> Nullable<Bigint>,
        #[max_length = 255]
        xml_checksum -> Nullable<Varchar>,
        xml_metadata -> Nullable<Text>,
        jade_file_pair_id -> Nullable<Bigint>,
    }
}

diesel::table! {
    jade_host (jade_host_id) {
        jade_host_id -> Bigint,
        allow_job_claim -> Nullable<Bit>,
        allow_job_work -> Nullable<Bit>,
        allow_open_job_claim -> Nullable<Bit>,
        date_created -> Nullable<Datetime>,
        date_heartbeat -> Nullable<Datetime>,
        date_updated -> Nullable<Datetime>,
        #[max_length = 255]
        host_name -> Nullable<Varchar>,
        satellite_capable -> Nullable<Bit>,
        version -> Nullable<Bigint>,
    }
}

diesel::table! {
    jade_map_bundle_to_file_pair (jade_bundle_id, jade_file_pair_order) {
        jade_bundle_id -> Bigint,
        jade_file_pair_id -> Bigint,
        jade_file_pair_order -> Integer,
    }
}

diesel::table! {
    jade_map_disk_to_file_pair (jade_disk_id, jade_file_pair_id) {
        jade_disk_id -> Bigint,
        jade_file_pair_id -> Bigint,
        jade_file_pair_order -> Integer,
    }
}

diesel::table! {
    jade_perf_data (jade_perf_data_id) {
        jade_perf_data_id -> Bigint,
        date_created -> Nullable<Datetime>,
        date_updated -> Nullable<Datetime>,
        #[max_length = 255]
        perf_name -> Nullable<Varchar>,
        #[max_length = 255]
        perf_value -> Nullable<Varchar>,
        version -> Nullable<Bigint>,
    }
}

diesel::table! {
    jade_rudics_archive (jade_rudics_archive_id) {
        jade_rudics_archive_id -> Bigint,
        date_created -> Nullable<Datetime>,
        date_updated -> Nullable<Datetime>,
        #[max_length = 255]
        description -> Nullable<Varchar>,
        #[max_length = 255]
        i3ms_uri -> Nullable<Varchar>,
        version -> Nullable<Bigint>,
    }
}

diesel::table! {
    jade_stream_metadata (jade_stream_metadata_id) {
        jade_stream_metadata_id -> Bigint,
        #[max_length = 255]
        dif_category -> Nullable<Varchar>,
        #[max_length = 255]
        dif_data_center_email -> Nullable<Varchar>,
        #[max_length = 255]
        dif_data_center_name -> Nullable<Varchar>,
        date_created -> Nullable<Datetime>,
        date_updated -> Nullable<Datetime>,
        #[max_length = 255]
        dif_sensor_name -> Nullable<Varchar>,
        #[max_length = 255]
        dif_entry_title -> Nullable<Varchar>,
        #[max_length = 255]
        dif_parameters -> Nullable<Varchar>,
        #[max_length = 255]
        sensor_name -> Nullable<Varchar>,
        #[max_length = 255]
        dif_subcategory -> Nullable<Varchar>,
        #[max_length = 255]
        dif_technical_contact_email -> Nullable<Varchar>,
        #[max_length = 255]
        dif_technical_contact_name -> Nullable<Varchar>,
        version -> Nullable<Bigint>,
    }
}

diesel::table! {
    jade_tdrss_archive (jade_tdrss_archive_id) {
        jade_tdrss_archive_id -> Bigint,
        capacity -> Nullable<Bigint>,
        date_created -> Nullable<Datetime>,
        date_updated -> Nullable<Datetime>,
        #[max_length = 255]
        description -> Nullable<Varchar>,
        number_of_copies -> Nullable<Integer>,
        sptr_capacity -> Nullable<Bigint>,
        #[max_length = 255]
        sptr_host -> Nullable<Varchar>,
        #[max_length = 255]
        sptr_prefix -> Nullable<Varchar>,
        version -> Nullable<Bigint>,
        jade_credentials_id -> Nullable<Bigint>,
        #[max_length = 255]
        sptr_directory -> Nullable<Varchar>,
    }
}

diesel::joinable!(jade_bundle -> jade_host (jade_host_id));
diesel::joinable!(jade_data_stream -> jade_credentials (jade_credentials_id));
diesel::joinable!(jade_data_stream -> jade_stream_metadata (jade_stream_metadata_id));
diesel::joinable!(jade_data_stream_metrics -> jade_data_stream (jade_data_stream_id));
diesel::joinable!(jade_disk -> jade_host (jade_host_id));
diesel::joinable!(jade_disk_archival_record -> jade_disk (jade_disk_id));
diesel::joinable!(jade_disk_archival_record -> jade_file_pair (jade_file_pair_id));
diesel::joinable!(jade_disk_archival_record -> jade_host (jade_host_id));
diesel::joinable!(jade_file_pair -> jade_data_stream (jade_data_stream_id));
diesel::joinable!(jade_file_pair_metadata -> jade_file_pair (jade_file_pair_id));
diesel::joinable!(jade_map_bundle_to_file_pair -> jade_bundle (jade_bundle_id));
diesel::joinable!(jade_map_bundle_to_file_pair -> jade_file_pair (jade_file_pair_id));
diesel::joinable!(jade_map_disk_to_file_pair -> jade_disk (jade_disk_id));
diesel::joinable!(jade_map_disk_to_file_pair -> jade_file_pair (jade_file_pair_id));
diesel::joinable!(jade_tdrss_archive -> jade_credentials (jade_credentials_id));

diesel::allow_tables_to_appear_in_same_query!(
    jade_bundle,
    jade_contact,
    jade_credentials,
    jade_data_stream,
    jade_data_stream_metrics,
    jade_data_streams_json,
    jade_disk,
    jade_disk_archival_record,
    jade_disk_archive,
    jade_disk_label,
    jade_file_pair,
    jade_file_pair_metadata,
    jade_host,
    jade_map_bundle_to_file_pair,
    jade_map_disk_to_file_pair,
    jade_perf_data,
    jade_rudics_archive,
    jade_stream_metadata,
    jade_tdrss_archive,
);

//---------------------------------------------------------------------------
//---------------------------------------------------------------------------
//---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    // use super::*;

    #[test]
    fn test_always_succeed() {
        assert!(true);
    }
}
