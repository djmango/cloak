import requests
import asyncio
import os
import tiktoken
import asyncpg

from log_memories import format_memories
from concurrent.futures import ThreadPoolExecutor, as_completed
from datetime import datetime, timedelta
from dotenv import load_dotenv

load_dotenv()

def get_jwt_token():
    # put ur jwt here
    return os.getenv("JWT_TOKEN")

# Modify this function to include the JWT token
def generate_from_chat(base_url, user_id, max_samples=1000, samples_per_query=30, range=None):
    endpoint = f"{base_url}/memories/generate_from_chat"
    # Get JWT token
    token = get_jwt_token()
    
    # Prepare the payload
    print(f"== test for user {user_id} ==")
    payload = {
        "user_id": user_id
    }
    
    if max_samples is not None:
        payload["max_samples"] = max_samples
    
    if samples_per_query is not None:
        payload["samples_per_query"] = samples_per_query
    
    if range is not None:
        payload["range"] = range

    headers = {
        "Content-Type": "application/json",
        "Authorization": f"Bearer {token}"  # Add the JWT token to the headers
    }
    try:
        response = requests.post(endpoint, json=payload, headers=headers)
        response.raise_for_status()
        print(f"Status Code: {response.status_code}")
        print("Response:")
        print(response.text)
        return response
    except requests.exceptions.RequestException as e:
        print(f"An error occurred: {e}")
        return None

def count_memory_tokens(memories):
    enc = tiktoken.get_encoding("cl100k_base")
    total_tokens = sum(len(enc.encode(memory["content"])) for memory in memories)
    return total_tokens

def test_memory_increment(base_url, user_id, days_back=3):
    now = datetime.utcnow()
    # Ensure logs directory exists
    log_dir = os.path.join(os.path.dirname(os.path.realpath(__file__)), 'logs', 'memory_experiments')
    os.makedirs(log_dir, exist_ok=True)
    
    # Test case 1: Generate from beginning of time to 'days_back' days ago
    start_date = datetime.min  # Beginning of time
    end_date = now - timedelta(days=days_back)
    range_payload = [int((now - timedelta(days=30)).timestamp()), int(end_date.timestamp())]
    response = generate_from_chat(base_url, user_id, range=range_payload)
    print(f"Test case 1: {start_date} to {end_date} - {'Success' if response else 'Fail'}")
    log_response(f"beginning_to_{days_back}_days_ago", response, log_dir)
    
    # Test cases 2 to days_back+1: Generate between ranges
    for i in range(days_back):
        start_date = now - timedelta(days=days_back-i)
        end_date = now - timedelta(days=days_back-1-i)
        range_payload = [int(start_date.timestamp()), int(end_date.timestamp())]
        response = generate_from_chat(base_url, user_id, range=range_payload)
        print(f"Test case {i+2}: {start_date} to {end_date} - {'Success' if response else 'Fail'}")
        log_response(f"range_{days_back-i}_{days_back-1-i}_days_ago", response, log_dir)
    
    # Test case days_back+2: Generate for yesterday to now
    yesterday = now - timedelta(days=1)
    range_payload = [int(yesterday.timestamp()), int(now.timestamp())]
    response = generate_from_chat(base_url, user_id, range=range_payload)
    print(f"Test case {days_back+2}: {yesterday} to {now} - {'Success' if response else 'Fail'}")
    log_response("range_yesterday_to_now", response, log_dir)
    
    print("All test cases completed.")

def log_response(test_case, response, log_dir):
    log_file_path = os.path.join(log_dir, f'{test_case}.log')
    with open(log_file_path, 'w') as log_file:
        if response is None:
            log_file.write(f"Test case: {test_case}\nResponse: None\n")
        else:
            log_file.write(f"Test case: {test_case}\nStatus Code: {response.status_code}\n")
            memories = response.json()
            for memory in memories:
                log_file.write(f'{memory["content"]},{memory["grouping"]}\n')

async def get_all_users():
    load_dotenv()
    database_url = os.getenv('DATABASE_URL')
    
    if not database_url:
        raise ValueError("DATABASE_URL not found in .env file")

    async with asyncpg.create_pool(database_url) as pool:
        async with pool.acquire() as conn:
            query = "SELECT id, first_name, last_name, email, created_at, updated_at FROM users"
            rows = await conn.fetch(query)
            
            users = [
                {
                    'id': row['id'],
                    'first_name': row['first_name'],
                    'last_name': row['last_name'],
                    'email': row['email'],
                    'created_at': row['created_at'],
                    'updated_at': row['updated_at']
                }
                for row in rows
            ]
            
            return users
        
if __name__ == '__main__':
    # TODO replace base_url with cloak.i.inc
    base_url = "http://localhost:8000"
    batch_size = 500
    max_workers = 10  # Adjust this based on your system's capabilities

    # Run the asynchronous function to get all users
    users = asyncio.run(get_all_users())
    user_ids = [user['id'] for user in users]
    chunk = int(len(user_ids)*0.05)
    user_ids = user_ids[chunk:chunk+15]
    print(f"Found {len(user_ids)} users")

    def process_user(user_id):
        start_date = datetime(1970, 1, 1)  # Use Unix epoch start date
        range_payload = [int(start_date.timestamp()), int(datetime.utcnow().timestamp())]
        response = generate_from_chat(base_url, user_id, range=range_payload)
        formatted_memory = format_memories(response.text)
        return user_id, formatted_memory

    # Process users in batches
    for i in range(0, len(user_ids), batch_size):
        batch = user_ids[i:i+batch_size]
        print(f"Processing batch {i//batch_size + 1} ({len(batch)} users)")

        with ThreadPoolExecutor(max_workers=max_workers) as executor:
            futures = [executor.submit(process_user, user_id) for user_id in batch]
            
            for future in as_completed(futures):
                try:
                    user_id, result = future.result()
                    # You can process or store the result here if needed
                    # Create the directory if it doesn't exist
                    os.makedirs('logs/user_memories', exist_ok=True)
                    
                    # Write the result to a file
                    with open(f'logs/user_memories/{user_id}.txt', 'w') as f:
                        f.write(result)
                    
                    print(f"Wrote memories for user {user_id}")
                except Exception as e:
                    print(f"An error occurred: {e}")

        print(f"Finished processing batch {i//batch_size + 1}")

    print("All batches processed")

    