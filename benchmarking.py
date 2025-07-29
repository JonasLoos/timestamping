import requests
import base64

target_url = "http://localhost:3000"


import asyncio
import aiohttp
import secrets
import time

def generate_hashes(count):
    """Precalculate all hashes before starting the benchmark"""
    print(f"Generating {count:,} random hashes...")
    hashes = []
    for i in range(count):
        random_hash_bytes = secrets.token_bytes(64)
        random_hash_base64 = base64.b64encode(random_hash_bytes).decode('utf-8')
        hashes.append(random_hash_base64)

        if (i + 1) % 100000 == 0:
            print(f"Generated {i + 1:,} hashes...")

    print(f"Hash generation complete!")
    return hashes

async def send_hash_request(session, hash_data):
    """Send a pre-generated hash to the server"""
    async with session.post(f"{target_url}/add", json={"hash": hash_data}) as response:
        return await response.json()

async def worker(session, semaphore, hash_queue, completed_counter):
    """Worker that processes hashes from the queue"""
    while True:
        try:
            # Get next hash from queue (non-blocking)
            hash_data = hash_queue.get_nowait()
        except asyncio.QueueEmpty:
            break

        async with semaphore:
            await send_hash_request(session, hash_data)
            completed_counter['count'] += 1

            if completed_counter['count'] % 10000 == 0:
                elapsed = time.time() - completed_counter['start_time']
                rate = completed_counter['count'] / elapsed
                print(f"Completed {completed_counter['count']} requests. Rate: {rate:.2f} req/sec")

async def batch_requests(total_requests, concurrent_limit):
    # Precalculate all hashes
    hashes = generate_hashes(total_requests)

    # Create a queue with all hashes
    hash_queue = asyncio.Queue()
    for hash_data in hashes:
        hash_queue.put_nowait(hash_data)

    async with aiohttp.ClientSession() as session:
        semaphore = asyncio.Semaphore(concurrent_limit)
        completed_counter = {'count': 0, 'start_time': time.time()}

        # Create worker tasks that will process hashes from the queue
        workers = [
            worker(session, semaphore, hash_queue, completed_counter)
            for _ in range(concurrent_limit)
        ]

        # Wait for all workers to complete
        await asyncio.gather(*workers)

async def main():
    total_requests = 1_000_000
    concurrent_limit = 2

    print(f"Starting benchmark with {total_requests:,} total requests, {concurrent_limit} concurrent...")
    start = time.time()

    await batch_requests(total_requests, concurrent_limit)

    elapsed = time.time() - start
    rate = total_requests / elapsed
    print(f"\nCompleted in {elapsed:.2f} seconds")
    print(f"Average rate: {rate:.2f} requests/second")

if __name__ == "__main__":
    asyncio.run(main())
