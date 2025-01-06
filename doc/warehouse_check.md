# warehouse_check.md

## Checkfile 
The following query was run against the JADE database to generate the
contents of the checkfile:

    select
        CONCAT(archive_checksum, '  ', data_warehouse_path, '/', SUBSTRING(archive_file, 43, 10000)) as checkfile
    from
        jade_file_pair
    where
            date_created >= '2024-04-01'
        and date_created < '2024-07-01'
    order by
        jade_file_pair_id;

In order to prevent table graphics and column names, the query was executed
from the command line using `mysql`:

    mysql -u [username] -p[password] -h [hostname] -D [database_name] -e "
    select CONCAT(archive_checksum, '  ', data_warehouse_path, '/', SUBSTRING(archive_file, 43, 10000)) as checkfile
    from jade_file_pair
    where date_created >= '2024-04-01'
    and date_created < '2024-07-01'
    order by jade_file_pair_id
    " --batch --skip-column-names > checkfile_windows.txt

Unfortunately, the output of the `mysql` command contained Windows line
endings (\r\n = 0x0d 0x0a), and needed to be converted into a Unix friendly
format:

    tr -d '\r' < checkfile_windows.txt > checkfile.txt

With a proper checkfile, the split command was used to break the large
file down into human-scale work units.

    split --lines=1000 checkfile.txt check_work.
