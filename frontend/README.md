# File Hash Timestamping Frontend

A simple web interface for uploading files, calculating their SHA-512 hashes, and interacting with the timestamping backend.

## Features

- **File Upload**: Drag and drop or click to select files
- **Hash Calculation**: Automatic SHA-512 hash calculation using Web Crypto API
- **Hash Storage**: Add file hashes to the backend store
- **Hash Checking**: Check if a hash exists in the store
- **Manual Hash Input**: Enter hashes manually for checking
- **Real-time Results**: Live feedback with timestamps

## Usage

1. **Start the backend server** (from the project root):
   ```bash
   cargo run
   ```

2. **Open the frontend**:
   - Open `frontend/index.html` in your web browser
   - Or serve it with a local HTTP server:
     ```bash
     cd frontend
     python3 -m http.server 8080
     # Then visit http://localhost:8080
     ```

3. **Upload and process files**:
   - Drag and drop a file or click to select one
   - The file's SHA-512 hash will be calculated automatically
   - Click "Add Hash to Store" to store the hash
   - Click "Check if Hash Exists" to verify if it's already stored

4. **Manual hash checking**:
   - Enter a 128-character hexadecimal hash in the manual input
   - Click "Check Hash" to verify if it exists in the store

## Technical Details

- **Hash Algorithm**: SHA-512 (512-bit hashes)
- **API Endpoints**: 
  - `POST /add` - Add a hash to the store
  - `POST /check` - Check if a hash exists
- **CORS**: Configured to allow cross-origin requests from the frontend
- **File Size**: No practical limit (uses streaming for large files)

## Browser Compatibility

Requires a modern browser with support for:
- Web Crypto API (for SHA-512 hashing)
- Fetch API (for HTTP requests)
- ES6+ features

## Security Notes

- File hashing is done client-side using the Web Crypto API
- No file content is sent to the server, only the hash
- The backend validates hash format and length 