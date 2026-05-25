#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
    #define POPEN _popen
    #define PCLOSE _pclose
    #define RUST_EXE ".\\target\\release\\rem.exe"
    #define CARGO_CMD "cargo build --release --quiet 2>nul"
#else
    #define POPEN popen
    #define PCLOSE pclose
    #define RUST_EXE "./target/release/rem"
    #define CARGO_CMD "cargo build --release --quiet 2>/dev/null"
#endif

int main(int argc, char *argv[]) {
    FILE *build_status = POPEN(CARGO_CMD, "r");
    if (build_status == NULL) {
        fprintf(stderr, "Error: Failed to initiate cargo build.\n");
        return 1;
    }

    int return_code = PCLOSE(build_status);
    if (return_code != 0) {
        fprintf(stderr, "Error: Cargo build failed.\n");
        return return_code;
    }

    size_t cmd_len = strlen(RUST_EXE) + 1;
    for (int i = 1; i < argc; i++) {
        cmd_len += strlen(argv[i]) + 1;
    }

    char *command = malloc(cmd_len);
    if (command == NULL) {
        fprintf(stderr, "Error: Memory allocation failed.\n");
        return 1;
    }

    strcpy(command, RUST_EXE);
    for (int i = 1; i < argc; i++) {
        strcat(command, " ");
        strcat(command, argv[i]);
    }

    int exit_status = system(command);

    free(command);
    return exit_status;
}
