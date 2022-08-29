# rust-cranker-connector

Cranker connector with Rust


    Browser      |   DMZ        |    Internal Network
     GET   --------> router <-------- connector ---> HTTP Service
                 |              |

# TODO

This project is still in development, below tasks needs to be completed before using it in production

- [x] heart beat for each websocket channel
- [ ] stop connector socket properly
- [ ] stop by calling wss deregister end point
- [ ] proper error handling
- [ ] graceful shutdown

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
    curl -k -vvv -X POST -H "Content-Type: application/json" -d '{"username":"amy"}' https://127.0.0.1:12000/post

    # GET
    curl -k -H "Content-Type: application/json" https://localhost:12000/hello 
   
    # POST
    curl -k -X POST -H "Content-Type: application/json" -d '{"username":"amy"}' https://localhost:12000/post
   
    # send batch
    for i in {1..100} 
    do
    curl -k -vvv -X POST -H "Content-Type: application/json" -d '{"username":"amy"}' https://127.0.0.1:12000/post
    done
    ```


# Release
