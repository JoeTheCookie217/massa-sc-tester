import { stringToBytes } from "@massalabs/as-types";
import { generateEvent } from "@massalabs/massa-as-sdk";

export function main(_: StaticArray<u8>): void {
    generateEvent("Hello, world!");
    generateEvent("Calling the main function");
}

export function receive(_: StaticArray<u8>): StaticArray<u8> {
    return stringToBytes("received");
}
