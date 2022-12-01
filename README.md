# ag_file_system_scanner

[![GitHub issues](https://img.shields.io/github/issues/Aditya-A-garwal/ag_file_system_scanner)](https://github.com/Aditya-A-garwal/ag_file_system_scanner/issues)
[![GitHub forks](https://img.shields.io/github/forks/Aditya-A-garwal/ag_file_system_scanner)](https://github.com/Aditya-A-garwal/ag_file_system_scanner/network)
[![GitHub stars](https://img.shields.io/github/stars/Aditya-A-garwal/ag_file_system_scanner)](https://github.com/Aditya-A-garwal/ag_file_system_scanner/stargazers)
[![GitHub license](https://img.shields.io/github/license/Aditya-A-garwal/ag_file_system_scanner)](https://github.com/Aditya-A-garwal/ag_file_system_scanner)

This is a high performance, nifty, command-line tool written in rust to navigate through the filesystem. It can be used to -

- Find Directories, Symlinks and Files by their full/partial name.
- Find the sizes of Directories recursively.
- Find permissions of Filesystem Entries (POSIX-style permissions only).
- General Navigation and exploration of the filesystem through the command-line.

It is a successor to [AgFileSystemScanner](https://github.com/Aditya-A-garwal/AgFileSystemScanner), which is the same tool written in C++. Rust was used over C++ due to its verbose, compile-time error handling. Exceptions are not present in rust, which reduces the chances of crashes happening.

## Usage

    fss [PATH] [options] [-r [DEPTH]] [-S|--search|--search-noext|--contains PATTERN]

## Options

    -r, --recursive             Recursively scan directories (can be followed by a positive integer to indicate the depth)
    -p, --permissions           Show permissions of all entries
    -t, --modification-time     Show time of last modification of entries

    -f, --files                 Show Regular Files (normally hidden)
    -l, --symlinks              Show Symlinks (normally hidden)
    -s, --special               Show Special Files such as sockets, pipes, etc. (normally hidden)

    -d, --dir-size              Recursively calculate and display the size of each directory

    -a, --abs                   Show the absolute path of each entry without any indentation

    -S, --search                Only show entries whose name completely matches the following string completely
        --search-noext          Only show entries whose name(except for the extension) completely matches the following string completely
        --contains              Only show entries whose name contains the following string completely

    -e, --show-err              Show errors
    -h, --help                  Print Usage Instructions

```PATH``` is the path to the directory from which to start the scan.

Only one of the search options(```-S```, ```--search```, ```--search-noext```, ```--contains```) can be set at a time.

The argument after the search flag is treated as the search pattern.

## Examples

Print the directories in the current directory, recursively going down two levels -

    fss -r 2

Print the contents of ```/proc```, including files, symlinks and special files with their permissions -

    fss "/proc" -f -l -s -p

Recursively search for all directories named ```proc``` in ```C://``` and show their sizes, last modification times -

    fss "C:/" -r -d -t -S "proc"
