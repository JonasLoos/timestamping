import asyncio
import aiohttp
import secrets
import time

target_url = "http://localhost:3427"

def generate_hashes(count):
    """Precalculate all hashes before starting the benchmark"""
    print(f"Generating {count:,} random hashes...")
    hashes = []
    # Generate all random bytes at once
    all_random_bytes = secrets.token_bytes(64 * count)
    # Process in chunks of 64 bytes
    for i in range(0, len(all_random_bytes), 64):
        chunk = all_random_bytes[i:i+64]
        hashes.append(chunk)

    print(f'Generated {count:,} hashes')
    print(f"Hash generation complete!")
    return hashes

async def send_batch_request(session, hashes_batch):
    """Send a batch of hashes to the server as raw bytes"""
    # Concatenate all hashes into a single byte array
    batch_bytes = b''.join(hashes_batch)

    async with session.post(
        f"{target_url}/add",
        data=batch_bytes,
        headers={"Content-Type": "application/octet-stream"}
    ) as response:
        return await response.json()

async def worker_batch(session, semaphore, hash_queue, completed_counter, batch_size=100):
    """Worker that processes hashes in batches from the queue"""
    while True:
        # Collect a batch of hashes
        batch = []
        for _ in range(batch_size):
            try:
                hash_data = hash_queue.get_nowait()
                batch.append(hash_data)
            except asyncio.QueueEmpty:
                break

        if not batch:
            break

        async with semaphore:
            try:
                await send_batch_request(session, batch)
                completed_counter['count'] += len(batch)
            except Exception as e:
                # Fallback to individual requests if batch fails
                print(f"Batch request failed: {e}")

        if completed_counter['count'] % 10000 == 0:
            elapsed = time.time() - completed_counter['start_time']
            rate = completed_counter['count'] / elapsed
            print(f"Completed {completed_counter['count']} requests. Rate: {rate:.2f} req/sec", end="\r")
    print()

async def batch_requests(hashes, concurrent_limit, batch_size=100):
    # Create a queue with all hashes
    hash_queue = asyncio.Queue()
    for hash_data in hashes:
        hash_queue.put_nowait(hash_data)

    async with aiohttp.ClientSession() as session:
        semaphore = asyncio.Semaphore(concurrent_limit)
        completed_counter = {'count': 0, 'start_time': time.time()}

        # Create worker tasks that will process hashes in batches from the queue
        workers = [
            worker_batch(session, semaphore, hash_queue, completed_counter, batch_size)
            for _ in range(concurrent_limit)
        ]

        # Wait for all workers to complete
        await asyncio.gather(*workers)

async def main():
    concurrent_limit = 2
    batch_size = 10000

    # Precalculate all hashes
    hashes = generate_hashes(5_000_000)

    print(f"Starting batch benchmark with {len(hashes):,} total requests, {concurrent_limit} concurrent, batch size {batch_size}...")
    start = time.time()

    await batch_requests(hashes, concurrent_limit, batch_size=batch_size)

    elapsed = time.time() - start
    rate = len(hashes) / elapsed
    print(f"\nBatch mode completed in {elapsed:.2f} seconds")
    print(f"Average rate: {rate:.2f} requests/second")

if __name__ == "__main__":
    asyncio.run(main())
