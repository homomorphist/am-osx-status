#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/socket.h>
#include <sys/un.h>

#define BUFFER_SIZE 1024

int main(int argc, char *argv[]) {
    if (argc != 2) {
        fprintf(stderr, "Usage: %s <socket_path>\n", argv[0]);
        exit(EXIT_FAILURE);
    }

    const char *socket_path = argv[1];
    int sockfd;
    struct sockaddr_un addr;
    char buffer[BUFFER_SIZE];
    ssize_t numRead, numWritten;

    // Create socket
    sockfd = socket(AF_UNIX, SOCK_STREAM, 0);
    if (sockfd == -1) {
        perror("socket");
        exit(EXIT_FAILURE);
    }

    // Set up the address structure
    memset(&addr, 0, sizeof(struct sockaddr_un));
    addr.sun_family = AF_UNIX;
    strncpy(addr.sun_path, socket_path, sizeof(addr.sun_path) - 1);

    // Connect to the socket
    if (connect(sockfd, (struct sockaddr *)&addr, sizeof(struct sockaddr_un)) == -1) {
        perror("connect");
        close(sockfd);
        exit(EXIT_FAILURE);
    }

    // Read from stdin and write to the socket
    while ((numRead = read(STDIN_FILENO, buffer, BUFFER_SIZE)) > 0) {
        numWritten = write(sockfd, buffer, numRead);
        if (numWritten != numRead) {
            perror("write");
            close(sockfd);
            exit(EXIT_FAILURE);
        }

        // Read response from the socket and write to stdout
        numRead = read(sockfd, buffer, BUFFER_SIZE);
        if (numRead == -1) {
            perror("read");
            close(sockfd);
            exit(EXIT_FAILURE);
        }

        if (write(STDOUT_FILENO, buffer, numRead) != numRead) {
            perror("write");
            close(sockfd);
            exit(EXIT_FAILURE);
        }
    }

    if (numRead == -1) {
        perror("read");
    }

    close(sockfd);
    return 0;
}
