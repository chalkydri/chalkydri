use sysinfo::Disks;

pub fn check_for_update_disk() {
    let disks = Disks::new_with_refreshed_list();
    for disk in disks.iter().filter(|d| d.is_removable()) {
        dbg!(disk.mount_point());
    }
}

