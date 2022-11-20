use std::env;
use std::fs;
use std::path;
use std::process;

/// Maximum allowed length of the provided path after which any further characters are ignored
const MAX_PATH_LEN: usize = 256;

#[cfg(target_family = "unix")]
/// Width of the string that contains the formatted last modified time of an entry
const FMT_TIME_WIDTH: usize = 20;

/// Maximum allowed length of the string that stores a formatted integer
const MAX_FMT_INT_LEN: usize = 32;

/// Number of spaces by which to further indent each subsequent nested directory's entries
const INDENT_COL_WIDTH: usize = 4;

/// Array of permissions strings indexed by mode value
const MODE_FMT: [&str; 8] = ["---", "--x", "-w-", "-wx", "r--", "r-x", "rw-", "rwx"];

/// Bitmask to contain the options set by the user
static mut OPTION_MASK: usize = 0;

/// Enumerates all the possible options that the user can provide from the command line
enum PrgOptions {
    /// Option that specifies if directories should be recursively scanned and displayed
    ShowRecursive = 0,
    /// Option that specified if the permissions of a filesystem entry should be printed
    ShowPermissions = 1,
    /// Option that specified if the last modification time of a file or directory should be printed
    ShowLasttime = 2,
    /// Option that specifies if the absolute paths of all entries should be printed without indentation
    ShowAbsnoindent = 3,
    /// Option that specifies if all files within a directory need to be individually displayed
    ShowFiles = 5,
    /// Option that specifies if all symlinks within a directory need to be individually displayed
    ShowSymlinks = 6,
    /// Option that specifies if all special files (such as sockets, block devices etc.) within a directory need to be individually displayed
    ShowSpecial = 7,
    /// Option that specifies if only those entries whose name matches a given pattern should be shown
    SearchExact = 8,
    /// Option that specifies if only those entries whose name (without the extension) matches a given pattern should be shown
    SearchNoext = 9,
    /// Option that specifies if only those entries whose name contains a given pattern should be shown
    SearchContains = 10,
    /// Option that specifies if directory sizes should be recursively calculated and shown
    ShowDirSize = 11,
    /// Option that species if errors should be shown
    ShowErrors = 12,
    /// Option that specifies if usage instructions need to be printed
    Help = 13,
}
/// Enumerates all the special file types, or not applicable
#[derive(PartialEq)]
enum SpecialFileType {
    Socket,
    BlockDevice,
    CharDevice,
    Fifo,
    NA,
}

/// Structure to store the counts of different types of filesystem entries
struct EntryCounter {
    /// Number of regular files (binary and text)
    _num_files: u64,
    /// Number of symlinks
    _num_symlinks: u64,
    /// Number of special files. A special file is any of the following -
    /// - block device
    /// - character device
    /// - FIFO pipe
    /// - Socket
    _num_special: u64,
    /// Number of directories
    _num_dirs: u64,
}

impl EntryCounter {
    /// Returns a new Instance of [`EntryCounter`](EntryCounter) with the counts of all entries set to 0
    fn new() -> EntryCounter {
        return EntryCounter {
            _num_files: 0,
            _num_symlinks: 0,
            _num_special: 0,
            _num_dirs: 0,
        };
    }

    /// Returns the number of regular files that have been counted
    fn get_file_cnt(&self) -> u64 {
        return self._num_files;
    }

    /// Returns the number of symlinks that have been counted
    fn get_symlink_cnt(&self) -> u64 {
        return self._num_symlinks;
    }

    /// Returns the number of special files that have been counted (see [this](EntryCounter)) for details on what should constitute a special file)
    fn get_special_cnt(&self) -> u64 {
        return self._num_special;
    }

    /// Returns the number of directories counted
    fn get_dir_cnt(&self) -> u64 {
        return self._num_dirs;
    }

    /// Returns the total number of entries counted
    fn get_entry_cnt(&self) -> u64 {
        return self._num_files + self._num_symlinks + self._num_special + self._num_dirs;
    }

    /// Increments the count of regular files by the specified value
    ///
    /// # Arguments
    ///
    /// - `p_inc_amt` - the amount by which to increase the count
    fn inc_file_cnt(&mut self, p_inc_amt: u64) {
        self._num_files += p_inc_amt;
    }

    /// Decrements the count of regular files by the specified value
    ///
    /// # Arguments
    ///
    /// - `p_dec_amt` - the amount by which to decrease the count
    fn dec_file_cnt(&mut self, p_dec_amt: u64) {
        self._num_files -= p_dec_amt;
    }

    /// Increments the count of symlinks by the specified value
    ///
    /// # Arguments
    ///
    /// - `p_inc_amt` - the amount by which to increase the count
    fn inc_symlink_cnt(&mut self, p_inc_amt: u64) {
        self._num_symlinks += p_inc_amt;
    }

    /// Decrements the count of symlinks by the specified value
    ///
    /// # Arguments
    ///
    /// - `p_dec_amt` - the amount by which to decrease the count
    fn dec_symlink_cnt(&mut self, p_dec_amt: u64) {
        self._num_symlinks -= p_dec_amt;
    }

    /// Increments the count of special files (see [this](EntryCounter) for details on what should constitute a special file) by the specified value
    ///
    /// # Arguments
    ///
    /// - `p_inc_amt` - the amount by which to increase the count
    fn inc_special_cnt(&mut self, p_inc_amt: u64) {
        self._num_special += p_inc_amt;
    }

    /// Decrements the count of special files (see [this](EntryCounter) for details on what should constitute a special file) by the specified value
    ///
    /// # Arguments
    ///
    /// - `p_dec_amt` - the amount by which to decrease the count
    fn dec_special_cnt(&mut self, p_dec_amt: u64) {
        self._num_special -= p_dec_amt;
    }

    /// Increments the count of directories by the specified value
    ///
    /// # Arguments
    ///
    /// - `p_inc_amt` - the amount by which to increase the count
    fn inc_dir_cnt(&mut self, p_inc_amt: u64) {
        self._num_dirs += p_inc_amt;
    }

    /// Decrements the count of directories by the specified value
    ///
    /// # Arguments
    ///
    /// - `p_dec_amt` - the amount by which to decrease the count
    fn dec_dir_cnt(&mut self, p_dec_amt: u64) {
        self._num_dirs -= p_dec_amt;
    }
}

#[cfg(target_family = "unix")]
/// Prints the permissions of a filesystem entry given the metadata
///
/// # Arguments
///
/// - `metadata` - metadata of the entry whose permissions need to be printed
macro_rules! print_permissions {
    ($metadata:ident) => {
        use std::os::unix::fs::PermissionsExt;

        // get the raw bits representing the permissions of the entry
        let mode = $metadata.permissions().mode() as usize;

        unsafe {
            // for each user, group and other, there are 7 possible modes
            // each mode has a unique representation of characters
            // use an array of string slices to store what is to be printed
            // for each of the 7 possible values
            print!(
                "{}{}{}   ",
                MODE_FMT.get_unchecked((mode >> 6) & 7),
                MODE_FMT.get_unchecked((mode >> 3) & 7),
                MODE_FMT.get_unchecked((mode >> 0) & 7)
            )
        }
    };
}

#[cfg(target_family = "unix")]
/// Prints the modification time of a filesystem entry
///
/// # Arguments
///
/// - `metadata` - metadata of the entry whose permissions are to be printed
/// - `path` - path of the entry (used in the error message if the time could not be read)
macro_rules! print_modif_time {
    ($metadata:ident, $path:expr) => {
        let Ok(time) = $metadata.modified() else {
                    if get_option(PrgOptions::ShowErrors) {
                        eprint!("Error while getting last modified time of \"{}\"\n", $path);
                    }
                    return true;
                };

        let time = Into::<chrono::DateTime<chrono::offset::Local>>::into(time);
        print!("{:>FMT_TIME_WIDTH$}", time.format("%b %d %Y  %H:%M"));
    };
}

/// Sets the given option in a mask (has not effect if the option is already set)
///
/// # Arguments
///
/// - `p_option_mask` - stores each option as a single bit in the bitmask
/// - `p_bit` - the index of the bit/option to be set
fn set_option(p_bit: PrgOptions) {
    unsafe {
        OPTION_MASK |= 1usize << (p_bit as usize);
    }
}

/// Returns the state of the given option from a mask
///
/// # Arguments
///
/// - `p_option_mask` - stores each option as a single bit in the bitmask
/// - `p_bit` - the index of the bit/option to check
///
/// # Returns
///
/// `True` if the option is set, `False` otherwise
fn get_option(p_bit: PrgOptions) -> bool {
    unsafe { OPTION_MASK & (1usize << (p_bit as usize)) != 0 }
}

/// Clears the given option in a mask (has not effect if the option is already unset)
///
/// # Arguments
///
/// - `p_option_mask` - stores each option as a single bit in the bitmask
/// - `p_bit` - the index of the bit/option to be set
#[allow(dead_code)]
fn clear_option(p_bit: PrgOptions) {
    unsafe {
        OPTION_MASK &= !(1usize << (p_bit as usize));
    }
}

/// Returns an &str slice that contains the given integer formatted with the thousands seperator
///
/// # Arguments
///
/// - `p_number` - unsigned number to format with thousands seperators
fn int_to_formatted_slice<T>(mut p_number: T) -> &'static str
where
    T: std::ops::Div<u64, Output = T>
        + std::ops::Rem<u64, Output = u64>
        + std::cmp::PartialOrd<u64>
        + Copy,
{
    unsafe {
        /// buffer to hold integer formatted with periods as a UTF-8 string
        static mut BUFF: [u8; MAX_FMT_INT_LEN] = [0; MAX_FMT_INT_LEN];

        /// stores digits of the given value as they are extracted
        static mut D: u64 = 0;

        /// length of the UTF-8 string after it is formed
        static mut BUFF_LEN: usize = 0;

        BUFF_LEN = 0;

        if p_number == 0u64 {
            BUFF[BUFF_LEN] = '0' as u8;
            BUFF_LEN += 1;
        }

        while p_number != 0u64 {
            D = p_number % 10u64;
            p_number = p_number / 10u64;

            BUFF[BUFF_LEN] = (D + ('0' as u64)) as u8;
            BUFF_LEN += 1;

            if (BUFF_LEN % 4) == 3 && p_number != 0 {
                BUFF[BUFF_LEN] = ',' as u8;
                BUFF_LEN += 1;
            }
        }

        for i in 0..(BUFF_LEN / 2) {
            (BUFF[i], BUFF[BUFF_LEN - i - 1]) = (BUFF[BUFF_LEN - i - 1], BUFF[i]);
        }

        return &std::str::from_utf8_unchecked(&BUFF)[..BUFF_LEN];
    }
}

/// Recursively calculates the size of a directory and returns it within an [Option<u64>]
///
/// If the size of a subdirectory/file within could not be calculated, it returns [None
///
/// # Arguments
///
/// - `p_option_mask`
fn calc_dir_size(p_init_dir_path: &path::Path, p_dir_path: &path::Path) -> Option<u64> {
    let entries = match fs::read_dir(&p_dir_path) {
        Ok(values) => values,
        Err(error) => {
            if get_option(PrgOptions::ShowErrors) {
                eprint!(
                    "Error while traversing {} while calculating size of directory {}\n{}\n",
                    p_dir_path.to_string_lossy(),
                    p_init_dir_path.to_string_lossy(),
                    error
                );
            }
            return None;
        }
    };

    let mut res: u64 = 0;

    for entry in entries {

        // if the current enty could not be read, silently skip it
        let Ok(entry) = entry else {
            continue;
        };

        let path_os = entry.path();

        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(error) => {
                if get_option(PrgOptions::ShowErrors) {
                    eprint!(
                        "Error while getting metadata of {} while calculating size of directory {}\n{}\n",
                        path_os.to_string_lossy(),
                        p_init_dir_path.to_string_lossy(),
                        error
                    );
                }
                return None;
            }
        };

        if metadata.is_symlink() {
            continue;
        }

        // if the entry is a file, then simply add its length to the result
        // if it is a directory, try to recursively calculate its size and add it to the result
        if metadata.is_file() {
            res += metadata.len();
        } else if metadata.is_dir() {
            let dir_size = match calc_dir_size(&p_init_dir_path, &path_os) {
                Some(dir_size) => dir_size,
                None => {
                    return None;
                }
            };

            res += dir_size;
        }
    }

    return Some(res);
}

/// Prints a symlink without indentation
///
/// Returns `false` if the symlink could be logged, `true` otherwise
///
/// # Arguments
///
/// - `p_path_os` - reference to the entry's path
/// - 'p_is_dir' - whether the target of the symlink is a directory or not
fn show_symlink_noindent(
    p_metadata: &fs::Metadata,
    p_path_os: &path::Path,
    p_is_dir: bool,
) -> bool {
    // borrow the filename (silently skip the current entry if this could not be done)
    let path = p_path_os.to_string_lossy();

    // get the canonicalized path name (print the error and exit if this could not be done)
    let dest_path = match p_path_os.canonicalize() {
        Ok(dest_path) => dest_path,
        Err(error) => {
            if get_option(PrgOptions::ShowErrors) {
                eprint!(
                    "Error while reading target of symlink \"{}\"\n{}\n",
                    path, error
                );
            }
            return true;
        }
    };

    if get_option(PrgOptions::ShowPermissions) {
        print_permissions!(p_metadata);
    }

    if get_option(PrgOptions::ShowLasttime) {
        print_modif_time!(p_metadata, path);
    }

    // if the target is a directory, enclose the symlink and target within angle brackets <>
    if p_is_dir {
        print!(
            "{:>20}    <{}> -> <{}>\n",
            "SYMLINK",
            path,
            dest_path.to_string_lossy()
        );
    } else {
        print!(
            "{:>20}    {} -> {}\n",
            "SYMLINK",
            path,
            dest_path.to_string_lossy()
        );
    }

    return false;
}

/// Prints a symlink with indentation
///
/// Returns `false` if the symlink could be logged, true otherwise
///
/// # Arguments
///
/// - 'p_indent_width' - number of spaces to leave before printing the entry
/// - `p_path_os` - reference to the entry's path
/// - 'p_is_dir' - whether the target of the symlink is a directory or not
fn show_symlink(
    p_indent_width: usize,
    p_metadata: &fs::Metadata,
    p_path_os: &path::Path,
    p_is_dir: bool,
) -> bool {
    // borrow the filename (silently skip the current entry if this could not be done)
    let Some(path) = p_path_os.file_name() else {
        return true;
    };

    // get the canonicalized path name
    let dest_path = match p_path_os.canonicalize() {
        Ok(dest_path) => dest_path,
        Err(error) => {
            if get_option(PrgOptions::ShowErrors) {
                eprint!(
                    "Error while reading target of symlink \"{}\"\n{}\n",
                    path.to_string_lossy(),
                    error
                );
            }
            return true;
        }
    };

    if get_option(PrgOptions::ShowPermissions) {
        print_permissions!(p_metadata);
    }

    if get_option(PrgOptions::ShowLasttime) {
        print_modif_time!(p_metadata, path.to_string_lossy());
    }

    // if the target is a directory, enclose the symlink and the target within angled brackets <>
    if p_is_dir {
        print!(
            "{:>20}    {:p_indent_width$}<{}> -> <{}>\n",
            "SYMLINK",
            "",
            path.to_string_lossy(),
            dest_path.to_string_lossy()
        );
    } else {
        print!(
            "{:>20}    {:p_indent_width$}{} -> {}\n",
            "SYMLINK",
            "",
            path.to_string_lossy(),
            dest_path.to_string_lossy()
        );
    }

    return false;
}

/// Prints a file without indentation
///
/// Returns `false` if the file could be logged, `true` otherwise
///
/// # Arguments
///
/// - 'p_indent_width' - number of spaces to leave before printing the entry
/// - `p_path_os` - reference to the entry's path
/// - 'p_file_len' - length of the file (in bytes)
fn show_file_noindent(p_metadata: &fs::Metadata, p_path_os: &path::Path, p_file_len: &u64) -> bool {
    let Ok(path) = p_path_os.canonicalize() else {
        return true;
    };

    if get_option(PrgOptions::ShowPermissions) {
        print_permissions!(p_metadata);
    }

    if get_option(PrgOptions::ShowLasttime) {
        print_modif_time!(p_metadata, path.to_string_lossy());
    }

    print!(
        "{:>20}    {}\n",
        int_to_formatted_slice(*p_file_len),
        path.to_string_lossy()
    );

    return false;
}

/// Prints a file with indentation
///
/// Returns `false` if the file could be logged, `true` otherwise
///
/// # Arguments
///
/// - 'p_indent_width' - number of spaces to leave before printing the entry
/// - `p_path_os` - reference to the entry's path
/// - 'p_file_len' - length of the file (in bytes)
fn show_file(p_indent_width: usize, p_metadata: &fs::Metadata, p_path_os: &path::Path) -> bool {
    let Some(path) = p_path_os.file_name() else {
        return true;
    };

    if get_option(PrgOptions::ShowPermissions) {
        print_permissions!(p_metadata);
    }

    if get_option(PrgOptions::ShowLasttime) {
        print_modif_time!(p_metadata, path.to_string_lossy());
    }

    print!(
        "{:>20}    {:p_indent_width$}{}\n",
        int_to_formatted_slice(p_metadata.len()),
        "",
        path.to_string_lossy()
    );

    return false;
}

/// Prints a directory without indentation
///
/// Returns `false` if the directory could be logged, `true` otherwise
///
/// # Arguments
///
/// - `p_path_os` - reference to the entry's path
fn show_dir_noindent(p_metadata: &fs::Metadata, p_path_os: &path::Path) -> bool {
    let Ok(path) = p_path_os.canonicalize() else {
        return true;
    };

    // see if the directory size needs to be printed (if yes, then check if it can be calculated)
    let sz = if get_option(PrgOptions::ShowDirSize) {
        if let Some(size) = calc_dir_size(&p_path_os, &p_path_os) {
            int_to_formatted_slice(size)
        } else {
            "ERROR"
        }
    } else {
        ""
    };

    if get_option(PrgOptions::ShowPermissions) {
        print_permissions!(p_metadata);
    }

    if get_option(PrgOptions::ShowLasttime) {
        print_modif_time!(p_metadata, path.to_string_lossy());
    }

    print!("{:>20}    <{}>\n", sz, path.to_string_lossy());

    return false;
}

/// Prints a directory with indentation
///
/// Returns `false` if the directory could be logged, `true` otherwise
///
/// # Arguments
///
/// - 'p_indent_width' - number of spaces to leave before printing the entry
/// - `p_path_os` - reference to the entry's path
fn show_dir(p_indent_width: usize, p_metadata: &fs::Metadata, p_path_os: &path::Path) -> bool {
    let Some(path) = p_path_os.file_name() else {
        return true;
    };

    // see if the directory size needs to be printed (if yes, then check if it can be calculated)
    // if it need not be printed, simply put an empty string
    // if it needs to be printed and can be calculated, format and print it
    // it if needs to be printed and can not be calculated, print ERROR
    let sz = if get_option(PrgOptions::ShowDirSize) {
        if let Some(size) = calc_dir_size(&p_path_os, &p_path_os) {
            int_to_formatted_slice(size)
        } else {
            "ERROR"
        }
    } else {
        ""
    };

    if get_option(PrgOptions::ShowPermissions) {
        print_permissions!(p_metadata);
    }

    if get_option(PrgOptions::ShowLasttime) {
        print_modif_time!(p_metadata, path.to_string_lossy());
    }

    print!(
        "{:>20}    {:p_indent_width$}<{}>\n",
        sz,
        "",
        path.to_string_lossy()
    );

    return false;
}

/// Prints a special file without indentation
///
/// Returns `false` if the special file could be logged, `true` otherwise
///
/// # Arguments
///
/// - `p_path_os` - reference to the entry's path
fn show_special_noindent(
    p_metadata: &fs::Metadata,
    p_path_os: &path::Path,
    p_special_file_type: &SpecialFileType,
) -> bool {
    let Ok(path) = p_path_os.canonicalize() else {
        return true;
    };

    let special_type = match p_special_file_type {
        SpecialFileType::Socket => "SOCKET",
        SpecialFileType::BlockDevice => "BLOCK DEVICE",
        SpecialFileType::CharDevice => "CHAR DEVICE",
        SpecialFileType::Fifo => "FIFO PIPE",
        _ => "SPECIAL",
    };

    if get_option(PrgOptions::ShowPermissions) {
        print_permissions!(p_metadata);
    }

    if get_option(PrgOptions::ShowLasttime) {
        print_modif_time!(p_metadata, path.to_string_lossy());
    }

    print!("{:>20}    {}\n", special_type, path.to_string_lossy());
    return false;
}

/// Prints a directory with indentation
///
/// Returns `false` if the special file could be logged, `true` otherwise
///
/// # Arguments
///
/// - 'p_indent_width' - number of spaces to leave before printing the entry
/// - `p_path_os` - reference to the entry's path
fn show_special(
    p_indent_width: usize,
    p_metadata: &fs::Metadata,
    p_path_os: &path::Path,
    p_special_file_type: &SpecialFileType,
) -> bool {
    let Some(path) = p_path_os.file_name() else {
        return true;
    };

    let special_type = match p_special_file_type {
        SpecialFileType::Socket => "SOCKET",
        SpecialFileType::BlockDevice => "BLOCK DEVICE",
        SpecialFileType::CharDevice => "CHAR DEVICE",
        SpecialFileType::Fifo => "FIFO PIPE",
        _ => "SPECIAL",
    };

    if get_option(PrgOptions::ShowPermissions) {
        print_permissions!(p_metadata);
    }

    if get_option(PrgOptions::ShowLasttime) {
        print_modif_time!(p_metadata, path.to_string_lossy());
    }

    print!(
        "{:>20}    {:p_indent_width$}{}\n",
        special_type,
        "",
        path.to_string_lossy()
    );
    return false;
}

/// Scans through directory given its path and prints its contents based on the flags given
///
/// Returns None on success and [`std::io::Error`](std::io::Error) if an error was encountered (propagates the error up the stack)
fn scan_path(
    p_entry_cnts_init: &mut EntryCounter,
    p_entry_cnts_full: &mut EntryCounter,
    p_max_level: &u64,
    p_level: usize,
    p_current_path: &path::Path,
) -> Option<std::io::Error> {
    // calculate the indent width to be used while printing the entries in the current directory
    let indent_width = INDENT_COL_WIDTH * p_level;
    // instantiate structure to hold the number of entries of each type in the current directory (not recursive)
    let mut cur_entry_cnts = EntryCounter::new();
    // total size of files in the current directory (only used when printing summary)
    let mut total_file_size: u64 = 0;

    // try to read the entries of the current directory
    // if the entries could not be iterated over (for example, due to insufficient permissions or the current entry being a file)
    // then return from the function and report this to the caller
    let entries = match fs::read_dir(&p_current_path) {
        Ok(values) => values,
        Err(error) => {
            return Some(error);
        }
    };

    for entry in entries {
        // if the current entry could not be found for some reason, then silently skip it
        let Ok(entry) = entry else {
            continue;
        };

        // get the metadata about this entry (will be used to query its type and in the case of regular files, its size)
        // if the metadata could not be queries, silently skip this entry
        let Ok(metadata) = entry.metadata() else {
            continue;
        };

        // get the path to the current entry
        let path_os = entry.path();

        // check for special file (on unix style operating systems, get the specific type as well)
        let special_file_type = if cfg!(target_family = "unix") {
            use std::os::unix::fs::FileTypeExt;

            if metadata.file_type().is_socket() {
                SpecialFileType::Socket
            } else if metadata.file_type().is_block_device() {
                SpecialFileType::BlockDevice
            } else if metadata.file_type().is_char_device() {
                SpecialFileType::CharDevice
            } else if metadata.file_type().is_fifo() {
                SpecialFileType::Fifo
            } else {
                SpecialFileType::NA
            }
        } else {
            SpecialFileType::NA
        };

        if metadata.is_symlink() {
            cur_entry_cnts.inc_symlink_cnt(1);

            // skip if the show symlinks option is not set
            if !get_option(PrgOptions::ShowSymlinks) {
                continue;
            }

            // depending on whether the absolute path (without indentation) needs to be printed,
            // try to print the current entry
            let failed = if get_option(PrgOptions::ShowAbsnoindent) {
                show_symlink_noindent(&metadata, &path_os, path_os.is_dir())
            } else {
                show_symlink(indent_width, &metadata, &path_os, path_os.is_dir())
            };

            // if the entry could not be printed, then remove its contribution from the counts
            if failed {
                cur_entry_cnts.dec_symlink_cnt(1);
            }
        } else if metadata.is_file() && special_file_type == SpecialFileType::NA {
            cur_entry_cnts.inc_file_cnt(1);

            // skip if the show files option is not set
            // since the number and size of files are aggregated at the end,
            // add it's size to the total file size
            if !get_option(PrgOptions::ShowFiles) {
                total_file_size += metadata.len();
                continue;
            }

            // depending on whether the absolute path (without indentation) needs to be printed,
            // try to print the current entry
            let failed = if get_option(PrgOptions::ShowAbsnoindent) {
                show_file_noindent(&metadata, &path_os, &metadata.len())
            } else {
                show_file(indent_width, &metadata, &path_os)
            };

            // if the entry could not be counted, then remove its contribution from the counts
            if failed {
                cur_entry_cnts.dec_file_cnt(1);
            }
        } else if metadata.is_dir() {
            cur_entry_cnts.inc_dir_cnt(1);

            // depending on whether the absolute path (without indentation) needs to be printed,
            // try to print the current entry
            let failed = if get_option(PrgOptions::ShowAbsnoindent) {
                show_dir_noindent(&metadata, &path_os)
            } else {
                show_dir(indent_width, &metadata, &path_os)
            };

            // if the entry could not be printed, then remove its contribution from the counts
            // otherwise, recursively print its contents if the show recursive option is set
            if failed {
                cur_entry_cnts.dec_dir_cnt(1);
            } else {
                if get_option(PrgOptions::ShowRecursive)
                    && (*p_max_level == 0u64 || p_level < (*p_max_level as usize))
                {
                    if let Some(error) = scan_path(
                        p_entry_cnts_init,
                        p_entry_cnts_full,
                        p_max_level,
                        1 + p_level,
                        &path_os,
                    ) {
                        if get_option(PrgOptions::ShowErrors) {
                            eprint!(
                                "Error while iterating over \"{}\"\n{}\n",
                                path_os.to_string_lossy(),
                                error
                            );
                        }
                    }
                }
            }
        } else {
            cur_entry_cnts.inc_special_cnt(1);

            if !get_option(PrgOptions::ShowSpecial) {
                continue;
            }

            // depending on whether the absolute path (without indentation) needs to be printed,
            // try to print the current entry
            let failed = if get_option(PrgOptions::ShowAbsnoindent) {
                show_special_noindent(&metadata, &path_os, &special_file_type)
            } else {
                show_special(indent_width, &metadata, &path_os, &special_file_type)
            };

            // if the entry could not be printed, remove its contribution from the counts
            if failed {
                cur_entry_cnts.dec_special_cnt(1);
            }
        }
    }

    // for the current directory, the summary needs to be printed for all the entries that were not supposed to be shown
    // for example, if the show files option is not set, the number of files along with their aggregated size needs
    // to be printed as a logical entry within the current directory
    // this is only to be done if the show absolute option is not set
    if !get_option(PrgOptions::ShowAbsnoindent) {

        // the total size of the files only needs to be printd if the show size option is set for directories
        // this is because the aggregated files are shown as a logical directory entry (as if the files were within another directory)
        // if the option was set, print the formatted size, otherwise print and empty string
        // for special file and symlink aggregate entries, an empty string needs to be printed if the show size option
        // is not set, and a - character need to be printed if the option is set
        let (file_sz, sz) = if get_option(PrgOptions::ShowDirSize) {
            (int_to_formatted_slice(total_file_size), '-')
        } else {
            ("", ' ')
        };

        // if the show files option is not set and there are special files, group them together and show the count
        if !get_option(PrgOptions::ShowFiles) && cur_entry_cnts.get_file_cnt() != 0 {
            if get_option(PrgOptions::ShowPermissions) {
                print!("            ");
            }
            if get_option(PrgOptions::ShowLasttime) {
                print!("{:FMT_TIME_WIDTH$}", ' ');
            }
            print!(
                "{:>20}    {:indent_width$}<{} files>\n",
                file_sz,
                "",
                int_to_formatted_slice(cur_entry_cnts.get_file_cnt())
            );
        }

        // if the show symlinks option is not set and there are special files, group them together and show the count
        if !get_option(PrgOptions::ShowSymlinks) && cur_entry_cnts.get_symlink_cnt() != 0 {
            if get_option(PrgOptions::ShowPermissions) {
                print!("            ");
            }
            if get_option(PrgOptions::ShowLasttime) {
                print!("{:FMT_TIME_WIDTH$}", ' ');
            }
            print!(
                "{:>20}    {:indent_width$}<{} symlinks>\n",
                sz,
                "",
                int_to_formatted_slice(cur_entry_cnts.get_symlink_cnt())
            );
        }

        // if the show special option is not set and there are special files, group them together and show the count
        if !get_option(PrgOptions::ShowSpecial) && cur_entry_cnts.get_special_cnt() != 0 {
            if get_option(PrgOptions::ShowPermissions) {
                print!("            ");
            }
            print!(
                "{:>20}    {:indent_width$}<{} special entries>\n",
                sz,
                "",
                int_to_formatted_slice(cur_entry_cnts.get_special_cnt())
            );
        }
    }

    // update the final and initial summaries with the current directory's traversal summary
    if p_level == 0 {
        p_entry_cnts_init.inc_symlink_cnt(cur_entry_cnts.get_symlink_cnt());
        p_entry_cnts_init.inc_file_cnt(cur_entry_cnts.get_file_cnt());
        p_entry_cnts_init.inc_dir_cnt(cur_entry_cnts.get_dir_cnt());
        p_entry_cnts_init.inc_special_cnt(cur_entry_cnts.get_special_cnt());
    }

    p_entry_cnts_full.inc_symlink_cnt(cur_entry_cnts.get_symlink_cnt());
    p_entry_cnts_full.inc_file_cnt(cur_entry_cnts.get_file_cnt());
    p_entry_cnts_full.inc_dir_cnt(cur_entry_cnts.get_dir_cnt());
    p_entry_cnts_full.inc_special_cnt(cur_entry_cnts.get_special_cnt());

    return None;
}

fn search_path(
    p_entry_cnts_match: &mut EntryCounter,
    p_entry_cnts_full: &mut EntryCounter,
    p_max_level: &u64,
    p_level: usize,
    p_current_path: &path::Path,
    p_search_path: &str,
) -> Option<std::io::Error> {
    // instantiate structure to hold the number of entries of each type in the current directory (not recursive)
    let mut cur_entry_cnts = EntryCounter::new();

    // try to read the entries of the current directory
    // if the entries could not be iterated over (for example, due to insufficient permissions or the current entry being a file)
    // then return from the function and report this to the caller
    let entries = match fs::read_dir(&p_current_path) {
        Ok(values) => values,
        Err(error) => {
            return Some(error);
        }
    };

    for entry in entries {
        // if the current entry could not be found for some reason, then silently skip it
        let Ok(entry) = entry else {
            continue;
        };

        // get the metadata about this entry (will be used to query its type and in the case of regular files, its size)
        // if the metadata could not be queries, silently skip this entry
        let Ok(metadata) = entry.metadata() else {
            continue;
        };

        // get the path to the current entry
        let path_os = entry.path();

        // check for special file
        let special_file_type = if cfg!(target_family = "unix") {
            use std::os::unix::fs::FileTypeExt;

            if metadata.file_type().is_socket() {
                SpecialFileType::Socket
            } else if metadata.file_type().is_block_device() {
                SpecialFileType::BlockDevice
            } else if metadata.file_type().is_char_device() {
                SpecialFileType::CharDevice
            } else if metadata.file_type().is_fifo() {
                SpecialFileType::Fifo
            } else {
                SpecialFileType::NA
            }
        } else {
            SpecialFileType::NA
        };

        let matches = if get_option(PrgOptions::SearchNoext) {
            // get the filename of this entry without the extension
            let Some(file_stem) = path_os.file_stem() else {
                continue;
            };
            let file_stem = file_stem.to_string_lossy();

            *file_stem == *p_search_path
        } else {
            // get the filename of this entry
            let Some(file_name) = path_os.file_name() else {
                continue;
            };
            let file_name = file_name.to_string_lossy();

            if get_option(PrgOptions::SearchExact) {
                *file_name == *p_search_path
            } else {
                file_name.contains(p_search_path)
            }
        };

        if metadata.is_symlink() {
            // skip if the show symlinks option is not set
            if !get_option(PrgOptions::ShowSymlinks) {
                cur_entry_cnts.inc_symlink_cnt(1);
                continue;
            }

            if !matches {
                cur_entry_cnts.inc_symlink_cnt(1);
                continue;
            }

            let failed = show_symlink_noindent(&metadata, &path_os, path_os.is_dir());

            if !failed {
                cur_entry_cnts.inc_symlink_cnt(1);
                p_entry_cnts_match.inc_symlink_cnt(1);
            }
        } else if metadata.is_file() && special_file_type == SpecialFileType::NA {
            if !get_option(PrgOptions::ShowFiles) {
                cur_entry_cnts.inc_file_cnt(1);
                continue;
            }

            if !matches {
                cur_entry_cnts.inc_file_cnt(1);
                continue;
            }

            let failed = show_file_noindent(&metadata, &path_os, &metadata.len());

            if !failed {
                cur_entry_cnts.inc_file_cnt(1);
                p_entry_cnts_match.inc_file_cnt(1);
            }
        } else if metadata.is_dir() {
            if !matches {
                cur_entry_cnts.inc_dir_cnt(1);
            } else {
                let failed = show_dir_noindent(&metadata, &path_os);

                if !failed {
                    cur_entry_cnts.inc_dir_cnt(1);
                    p_entry_cnts_match.inc_dir_cnt(1);
                }
            }

            if get_option(PrgOptions::ShowRecursive)
                && (*p_max_level == 0u64 || p_level < (*p_max_level as usize))
            {
                if let Some(error) = search_path(
                    p_entry_cnts_match,
                    p_entry_cnts_full,
                    p_max_level,
                    1 + p_level,
                    &path_os,
                    p_search_path,
                ) {
                    if get_option(PrgOptions::ShowErrors) {
                        eprint!(
                            "Error while iterating over \"{}\"\n{}\n",
                            path_os.to_string_lossy(),
                            error
                        );
                    }
                }
            }
        } else {
            if !get_option(PrgOptions::ShowSpecial) {
                cur_entry_cnts.inc_special_cnt(1);
                continue;
            }

            if !matches {
                cur_entry_cnts.inc_special_cnt(1);
                continue;
            }

            let failed = show_special_noindent(&metadata, &path_os, &special_file_type);

            if !failed {
                cur_entry_cnts.inc_special_cnt(1);
                p_entry_cnts_match.inc_special_cnt(1);
            }
        }
    }

    p_entry_cnts_full.inc_symlink_cnt(cur_entry_cnts.get_symlink_cnt());
    p_entry_cnts_full.inc_file_cnt(cur_entry_cnts.get_file_cnt());
    p_entry_cnts_full.inc_dir_cnt(cur_entry_cnts.get_dir_cnt());
    p_entry_cnts_full.inc_special_cnt(cur_entry_cnts.get_special_cnt());

    return None;
}

fn scan_path_init(p_init_path: &str, p_max_level: &u64) {
    // create new containers to store files in current directory and subdirectories respectively
    let mut entry_cnts_init = EntryCounter::new();
    let mut entry_cnts_full: EntryCounter = EntryCounter::new();

    // create a path object over the initial path
    let init_path = path::Path::new(&p_init_path);

    // check if the path could be iterated over
    // if an error occours (such as insufficient permissions, non-existant directory)
    // then report it and return without printing the summary of traversal
    if let Some(error) = scan_path(
        &mut entry_cnts_init,
        &mut entry_cnts_full,
        p_max_level,
        0,
        init_path,
    ) {
        print!(
            "Error while iterating over \"{}\"\n{}\n",
            p_init_path, error
        );
        return;
    }

    let file_cnt = int_to_formatted_slice(entry_cnts_init.get_file_cnt()).to_owned();
    let symlink_cnt = int_to_formatted_slice(entry_cnts_init.get_symlink_cnt()).to_owned();
    let special_cnt = int_to_formatted_slice(entry_cnts_init.get_special_cnt()).to_owned();
    let dir_cnt = int_to_formatted_slice(entry_cnts_init.get_dir_cnt()).to_owned();
    let total_cnt = int_to_formatted_slice(entry_cnts_init.get_entry_cnt()).to_owned();

    // Unformatted summary string for directory to traverse (not including subdirectories)
    print!(
        "\n\
            Summary of \"{}\"\n\
            <{} files>\n\
            <{} symlinks>\n\
            <{} special files>\n\
            <{} subdirectories>\n\
            <{} total entries>\n\
            \n",
        p_init_path, file_cnt, symlink_cnt, special_cnt, dir_cnt, total_cnt
    );

    // if the recursive traversal option was not set, then return without printing the complete summary
    if !get_option(PrgOptions::ShowRecursive) {
        return;
    }

    let file_cnt = int_to_formatted_slice(entry_cnts_full.get_file_cnt()).to_owned();
    let symlink_cnt = int_to_formatted_slice(entry_cnts_full.get_symlink_cnt()).to_owned();
    let special_cnt = int_to_formatted_slice(entry_cnts_full.get_special_cnt()).to_owned();
    let dir_cnt = int_to_formatted_slice(entry_cnts_full.get_dir_cnt()).to_owned();
    let total_cnt = int_to_formatted_slice(entry_cnts_full.get_entry_cnt()).to_owned();

    // Unformatted summary string for the directory to traverse (including subdirectories)
    print!(
        "Including subdirectories\n\
            <{} files>\n\
            <{} symlinks>\n\
            <{} special files>\n\
            <{} subdirectories>\n\
            <{} total entries>\n\
            \n",
        file_cnt, symlink_cnt, special_cnt, dir_cnt, total_cnt
    );
}

fn search_path_init(p_init_path: &str, p_search_path: &str, p_max_level: &u64) {
    let mut entry_cnts_match = EntryCounter::new();
    let mut entry_cnts_total: EntryCounter = EntryCounter::new();

    let init_path = path::Path::new(&p_init_path);

    if let Some(error) = search_path(
        &mut entry_cnts_match,
        &mut entry_cnts_total,
        p_max_level,
        0,
        &init_path,
        p_search_path,
    ) {
        if get_option(PrgOptions::ShowErrors) {
            eprint!(
                "Error while iterating over \"{}\"\n{}\n",
                p_init_path, error
            );
        }
        return;
    }

    let file_cnt = int_to_formatted_slice(entry_cnts_match.get_file_cnt()).to_owned();
    let symlink_cnt = int_to_formatted_slice(entry_cnts_match.get_symlink_cnt()).to_owned();
    let special_cnt = int_to_formatted_slice(entry_cnts_match.get_special_cnt()).to_owned();
    let dir_cnt = int_to_formatted_slice(entry_cnts_match.get_dir_cnt()).to_owned();
    let total_cnt = int_to_formatted_slice(entry_cnts_match.get_entry_cnt()).to_owned();

    // Unformatted summary string for number of entries found matching search pattern (in search mode)
    print!(
        "\n\
            Summary of matching entries\n\
            <{} files>\n\
            <{} symlinks>\n\
            <{} special files>\n\
            <{} subdirectories>\n\
            <{} total entries>\n\
            \n",
        file_cnt, symlink_cnt, special_cnt, dir_cnt, total_cnt
    );

    let file_cnt = int_to_formatted_slice(entry_cnts_total.get_file_cnt()).to_owned();
    let symlink_cnt = int_to_formatted_slice(entry_cnts_total.get_symlink_cnt()).to_owned();
    let special_cnt = int_to_formatted_slice(entry_cnts_total.get_special_cnt()).to_owned();
    let dir_cnt = int_to_formatted_slice(entry_cnts_total.get_dir_cnt()).to_owned();
    let total_cnt = int_to_formatted_slice(entry_cnts_total.get_entry_cnt()).to_owned();

    // Unformatted summary string for number of entries traversed while matching search pattern (in search mode)
    print!(
        "Summary of traversal of \"{}\"\n\
            <{} files>\n\
            <{} symlinks>\n\
            <{} special files>\n\
            <{} subdirectories>\n\
            <{} total entries>\n\
            \n",
        p_init_path, file_cnt, symlink_cnt, special_cnt, dir_cnt, total_cnt
    );
}

fn main() {
    // Path to start the scan process from
    let mut init_path: String = ".".to_owned();

    // Pattern to search for
    let mut search_path: String = "".to_owned();

    // whether the previous flag was "-r" or "--recursive"
    let mut specify_recur_depth: bool = false;

    let mut specify_search_path: bool = false;

    // maximum number of levels to recurse until if the PrgOptions::ShowRecursive option is set (a value of 0 denotes no limit)
    let mut max_recur_level: u64 = 0;

    for (i, arg) in env::args().enumerate().skip(1) {
        let arg_len = arg.len();

        if arg_len <= 0 {
            print!("Ignoring Unknown Option of length 0\n");
        }

        if arg.chars().nth(0).unwrap() != '-' {
            if specify_recur_depth {
                specify_recur_depth = false;
                if let Ok(depth) = arg.parse::<u64>() {
                    max_recur_level = depth;
                    if depth <= 0 {
                        print!("Maximum recursion depth must be greater than 0!\n");
                        print!("Ignoring recursive option\n");
                        clear_option(PrgOptions::ShowRecursive);
                    }
                    continue;
                } else {
                    print!("Could not convert \"{}\" to an integer\n", arg);
                    print!("Ignoring recursive option\n");
                    clear_option(PrgOptions::ShowRecursive);

                    continue;
                }
            } else if specify_search_path {
                search_path = arg.clone();
                continue;
            } else {
                init_path = arg.clone();
                if init_path.len() > MAX_PATH_LEN {
                    init_path = init_path[..MAX_PATH_LEN].to_owned();
                }
                continue;
            }
        }
        specify_recur_depth = false;
        specify_search_path = false;

        if arg == "-h" || arg == "--help" {
            set_option(PrgOptions::Help);
        } else if arg == "-e" || arg == "--show-err" {
            set_option(PrgOptions::ShowErrors);
        } else if arg == "-r" || arg == "--recursive" {
            set_option(PrgOptions::ShowRecursive);
            specify_recur_depth = true;
        } else if arg == "-f" || arg == "--files" {
            set_option(PrgOptions::ShowFiles);
        } else if arg == "-l" || arg == "--symlinks" {
            set_option(PrgOptions::ShowSymlinks);
        } else if arg == "-s" || arg == "--special" {
            set_option(PrgOptions::ShowSpecial);
        } else if arg == "-d" || arg == "--dir-size" {
            set_option(PrgOptions::ShowDirSize);
        } else if arg == "-a" || arg == "--abs" {
            set_option(PrgOptions::ShowAbsnoindent);
        } else if arg == "-S" || arg == "--search" {
            if get_option(PrgOptions::SearchNoext) || get_option(PrgOptions::SearchContains) {
                print!("Can only set one search mode at a time\n");
                print!("Terminating...");
                process::exit(-1);
            }

            specify_search_path = true;
            set_option(PrgOptions::SearchExact);

            if env::args().len() <= i + 1 {
                print!("No Search Pattern provided after {} flag\n", arg);
                process::exit(-1);
            }
        } else if arg == "--search-noext" {
            if get_option(PrgOptions::SearchExact) || get_option(PrgOptions::SearchContains) {
                print!("Can only set one search mode at a time\n");
                print!("Terminating...");
                process::exit(-1);
            }

            specify_search_path = true;
            set_option(PrgOptions::SearchNoext);

            if env::args().len() <= i + 1 {
                print!("No Search Pattern provided after {} flag\n", arg);
                process::exit(-1);
            }
        } else if arg == "--contains" {
            if get_option(PrgOptions::SearchNoext) || get_option(PrgOptions::SearchExact) {
                print!("Can only set one search mode at a time\n");
                print!("Terminating...");
                process::exit(-1);
            }

            specify_search_path = true;
            set_option(PrgOptions::SearchContains);

            if env::args().len() <= i + 1 {
                print!("No Search Pattern provided after {} flag\n", arg);
                process::exit(-1);
            }
        } else if cfg!(target_family = "unix") && (arg == "-p" || arg == "--permissions") {
            set_option(PrgOptions::ShowPermissions);
        } else if cfg!(target_family = "unix") && (arg == "-t" || arg == "--modification-time") {
            set_option(PrgOptions::ShowLasttime);
        } else {
            print!("Ignoring unknown option {}\n", arg);
        }
    }

    if get_option(PrgOptions::Help) {
        // Name of current process
        let process_name = std::env::args().nth(0).unwrap_or("fss".to_owned());

        #[cfg(target_family = "unix")]
        println!("\n\
        File System Scanner (dumblebots.com)\n\
        \n\
        Usage: {} [PATH] [options]\n\
        Scan through the filesystem starting from PATH.\n\
        \n\
        Example: {} \"..\" --recursive --files\n\
        \n\
        Options:\n\
        -r, --recursive             Recursively scan directories (can be followed by a positive integer to indicate the depth)\n\
        -p, --permissions           Show Permissions of all entries\n\
        -t, --modification-time     Show time of last modification of entries\n\
        \n\
        -f, --files                 Show Regular Files (normally hidden)\n\
        -l, --symlinks              Show Symlinks (normally hidden)\n\
        -s, --special               Show Special Files such as sockets, pipes, etc. (normally hidden)\n\
        \n\
        -d, --dir-size              Recursively calculate and display the size of each directory\n\
        \n\
        -a, --abs                   Show the absolute path of each entry without any indentation\n\
        \n\
        -S, --search                Only show entries whose name completely matches the following string completely\n    \
            --search-noext          Only show entries whose name(except for the extension) matches the following string completely\n    \
            --contains              Only show entries whose name contains the following string completely\n\
        \n\
        -e, --show-err              Show errors\n\
        -h, --help                  Print Usage Instructions\n\
        \n", &process_name, &process_name);

        #[cfg(not(target_family = "unix"))]
        println!("\n\
        File System Scanner (dumblebots.com)\n\
        \n\
        Usage: {} [PATH] [options]\n\
        Scan through the filesystem starting from PATH.\n\
        \n\
        Example: {} \"..\" --recursive --files\n\
        \n\
        Options:\n\
        -r, --recursive             Recursively scan directories (can be followed by a positive integer to indicate the depth)\n\
        \n\
        -f, --files                 Show Regular Files (normally hidden)\n\
        -l, --symlinks              Show Symlinks (normally hidden)\n\
        -s, --special               Show Special Files such as sockets, pipes, etc. (normally hidden)\n\
        \n\
        -d, --dir-size              Recursively calculate and display the size of each directory\n\
        \n\
        -a, --abs                   Show the absolute path of each entry without any indentation\n\
        \n\
        -S, --search                Only show entries whose name completely matches the following string completely\n    \
            --search-noext          Only show entries whose name(except for the extension) matches the following string completely\n    \
            --contains              Only show entries whose name contains the following string completely\n\
        \n\
        -e, --show-err              Show errors\n\
        -h, --help                  Print Usage Instructions\n\
        \n", &process_name, &process_name);

        process::exit(0);
    }

    if get_option(PrgOptions::SearchExact)
        || get_option(PrgOptions::SearchNoext)
        || get_option(PrgOptions::SearchContains)
    {
        search_path_init(&init_path, &search_path, &max_recur_level)
    } else {
        scan_path_init(&init_path, &max_recur_level);
    }
}
