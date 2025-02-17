#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>
#include <pthread.h>
#include <time.h>

#define MESSAGE "ping\n"
#define MESSAGE_LEN 5
#define BUFFER_SIZE 4096
#define ITERATIONS 99

void error(const char *msg) {
    perror(msg);
    exit(1);
}

typedef struct {
    const char *socket_path;
    int thread_id;
} thread_data_t;

void *client_thread(void *arg) {
    thread_data_t *data = (thread_data_t *)arg;
    const char *socket_path = data->socket_path;
    int thread_id = data->thread_id;
    int sockfd;
    struct sockaddr_un serv_addr;
    char buffer[BUFFER_SIZE];

    sockfd = socket(AF_UNIX, SOCK_STREAM, 0);
    if (sockfd < 0) {
        fprintf(stderr, "Thread %d: Error opening socket\n", thread_id);
        pthread_exit(NULL);
    }

    memset(&serv_addr, 0, sizeof(serv_addr));
    serv_addr.sun_family = AF_UNIX;
    strncpy(serv_addr.sun_path, socket_path, sizeof(serv_addr.sun_path) - 1);

    // Add a small delay before attempting to connect
    usleep(100000); // 100 milliseconds

    if (connect(sockfd, (struct sockaddr *)&serv_addr, sizeof(serv_addr)) < 0) {
        fprintf(stderr, "Thread %d: Error connecting to socket\n", thread_id);
        close(sockfd);
        pthread_exit(NULL);
    }

    printf("Thread %d connected to socket at %s\n", thread_id, socket_path);

    for (int i = 0; i < ITERATIONS; i++) {
        if (write(sockfd, MESSAGE, MESSAGE_LEN) < 0) {
            fprintf(stderr, "Thread %d: Error writing to socket at iteration %d\n", thread_id, i);
            close(sockfd);
            pthread_exit(NULL);
        }

        // Read response from server
        ssize_t n = read(sockfd, buffer, BUFFER_SIZE - 1);
        if (n < 0) {
            fprintf(stderr, "Thread %d: Error reading from socket at iteration %d\n", thread_id, i);
            close(sockfd);
            pthread_exit(NULL);
        }
        buffer[n] = '\0'; // Null-terminate the response

        // Print the response for debugging
        // printf("Thread %d received at iteration %d: %s\n", thread_id, i, buffer);
    }

    close(sockfd);
    printf("Thread %d finished\n", thread_id);
    pthread_exit(NULL);
}

int main(int argc, char *argv[]) {
    if (argc != 3) {
        fprintf(stderr, "Usage: %s <socket_path> <num_connections>\n", argv[0]);
        exit(1);
    }

    const char *socket_path = argv[1];
    int num_connections = atoi(argv[2]);
    pthread_t threads[num_connections];
    thread_data_t thread_data[num_connections];

    struct timespec start, end;
    clock_gettime(CLOCK_MONOTONIC, &start);

    for (int i = 0; i < num_connections; i++) {
        thread_data[i].socket_path = socket_path;
        thread_data[i].thread_id = i;
        if (pthread_create(&threads[i], NULL, client_thread, &thread_data[i]) != 0) {
            fprintf(stderr, "Error creating thread %d\n", i);
            exit(1);
        }
    }

    for (int i = 0; i < num_connections; i++) {
        pthread_join(threads[i], NULL);
    }

    clock_gettime(CLOCK_MONOTONIC, &end);

    double elapsed = (end.tv_sec - start.tv_sec) + (end.tv_nsec - start.tv_nsec) / 1e9;
    printf("Sent %d messages per connection in %.2f seconds\n", ITERATIONS, elapsed);
    printf("Average messages per second per connection: %.2f\n", ITERATIONS / elapsed);
    printf("Total messages sent: %d\n", ITERATIONS * num_connections);
    printf("Total average messages per second: %.2f\n", (ITERATIONS * num_connections) / elapsed);

    return 0;
}
