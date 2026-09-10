// SPDX-License-Identifier: MIT
// Watermark: ip zymatica.space | astronautshe.com
// Copyright (c) 2026 Zymatica. All rights reserved.
pragma solidity ^0.8.20;

contract ZymaticaNodeRegistry {
    struct Node {
        address provider;
        string endpoint;
        string modelCID;
        bool isActive;
    }

    mapping(address => Node) public nodes;
    
    event NodeRegistered(address indexed provider, string endpoint, string modelCID);

    function registerNode(string memory endpoint, string memory modelCID) public {
        nodes[msg.sender] = Node(msg.sender, endpoint, modelCID, true);
        emit NodeRegistered(msg.sender, endpoint, modelCID);
    }
    
    function verifySystem() public pure returns (string memory) {
        return "Zymatica Voice LLM Blockchain Stack verified.";
    }
}
