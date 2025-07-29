import requests

target_url = "http://localhost:3000"


import asyncio
import aiohttp
import secrets
import time

async def send_hash_request(session):
    # Generate random 64-byte hash
    random_hash = secrets.token_bytes(64).hex()
    
    async with session.post(f"{target_url}/add", json={"hash": random_hash}) as response:
        return await response.json()

async def worker(session, semaphore, total_requests, completed_counter):
    while completed_counter['count'] < total_requests:
        async with semaphore:
            if completed_counter['count'] >= total_requests:
                break
            await send_hash_request(session)
            completed_counter['count'] += 1
            
            if completed_counter['count'] % 10000 == 0:
                elapsed = time.time() - completed_counter['start_time']
                rate = completed_counter['count'] / elapsed
                print(f"Completed {completed_counter['count']} requests. Rate: {rate:.2f} req/sec")

async def batch_requests(total_requests, concurrent_limit):
    async with aiohttp.ClientSession() as session:
        semaphore = asyncio.Semaphore(concurrent_limit)
        completed_counter = {'count': 0, 'start_time': time.time()}
        
        # Create worker tasks that will continuously process requests
        workers = [
            worker(session, semaphore, total_requests, completed_counter)
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
