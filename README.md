# rust-cranker-connector

Cranker connector with Rust


    Browser      |   DMZ        |    Internal Network
     GET   --------> router <-------- connector ---> HTTP Service
                 |              |

# Development

1. start cranker from local first
    ```text
    # register: wss://localhost:16488/register?connectorId=abc
    # health: http://localhost:12438/health
    # http: https://localhost:9443
    ```

2. send request to cranker server via curl

    ```shell
    # POST
    curl -k -vvv -X POST -H "Content-Type: application/json"  -d '{"name": "linuxize", "email":"linuxize@example.com"}' https://localhost:9443/post-msg

    # GET
    curl -k -vvv https://localhost:9443/get-msg
    curl http://127.0.0.1:8080/io.txt
    curl -k https://127.0.0.1:9443/io.txt
   
    # POST
    curl -k -X POST -H "Content-Type: application/json" -d '{"username":"amy"}' https://127.0.0.1:9443/post
    ```
3. 

# Release
