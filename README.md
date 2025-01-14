### How it works

The program works in passive mode. It doesn't make any request if not called. When you make a request at /health it will perform these checks:
1. check if the node is in syncing state. If true, respond with "Ethereum node is syncing" and status code 503. If false, continue with the checks
2. if not in sync, the program will compare the local head block with the one from other public rpc api. If the difference is less than 10 blocks, the node is considerated healthy otherwise it will respond with "Ethereum node is behind reference nodes" and status code 503.

#### ToDo
- [ ] Add timestamp check
