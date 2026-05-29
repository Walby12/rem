#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
    #define RUST_EXE ".\\target\\release\\rem.exe"
#else
    #define RUST_EXE "./target/release/rem"
#endif

int main(int argc, char *argv[]) {
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
