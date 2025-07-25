// API base URL
const API_BASE_URL = 'http://127.0.0.1:3000';

// DOM elements
const fileInput = document.getElementById('fileInput');
const fileInfo = document.getElementById('fileInfo');
const fileName = document.getElementById('fileName');
const fileSize = document.getElementById('fileSize');
const fileHash = document.getElementById('fileHash');
const addHashBtn = document.getElementById('addHashBtn');
const checkHashBtn = document.getElementById('checkHashBtn');
const results = document.getElementById('results');
const manualHashInput = document.getElementById('manualHashInput');
const checkManualHashBtn = document.getElementById('checkManualHashBtn');

// Current file data
let currentFile = null;
let currentHash = null;

// Event listeners
fileInput.addEventListener('change', handleFileSelect);
addHashBtn.addEventListener('click', addHashToStore);
checkHashBtn.addEventListener('click', checkHashExists);
checkManualHashBtn.addEventListener('click', checkManualHash);

// Drag and drop functionality
const fileInputLabel = document.querySelector('.file-input-label');

fileInputLabel.addEventListener('dragover', (e) => {
    e.preventDefault();
    fileInputLabel.style.borderColor = '#667eea';
    fileInputLabel.style.background = '#f0f2ff';
});

fileInputLabel.addEventListener('dragleave', (e) => {
    e.preventDefault();
    fileInputLabel.style.borderColor = '#ddd';
    fileInputLabel.style.background = '#f8f9fa';
});

fileInputLabel.addEventListener('drop', (e) => {
    e.preventDefault();
    fileInputLabel.style.borderColor = '#ddd';
    fileInputLabel.style.background = '#f8f9fa';
    
    const files = e.dataTransfer.files;
    if (files.length > 0) {
        fileInput.files = files;
        handleFileSelect();
    }
});

async function handleFileSelect() {
    const file = fileInput.files[0];
    if (!file) {
        hideFileInfo();
        return;
    }

    currentFile = file;
    showFileInfo(file);
    
    try {
        currentHash = await calculateFileHash(file);
        fileHash.textContent = currentHash;
        enableButtons();
    } catch (error) {
        addResult('error', 'Error calculating hash', error.message);
        hideFileInfo();
    }
}

function showFileInfo(file) {
    fileName.textContent = file.name;
    fileSize.textContent = formatFileSize(file.size);
    fileInfo.style.display = 'block';
}

function hideFileInfo() {
    fileInfo.style.display = 'none';
    currentFile = null;
    currentHash = null;
    disableButtons();
}

function formatFileSize(bytes) {
    if (bytes === 0) return '0 Bytes';
    const k = 1024;
    const sizes = ['Bytes', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
}

async function calculateFileHash(file) {
    return new Promise((resolve, reject) => {
        const reader = new FileReader();
        reader.onload = async (e) => {
            try {
                const arrayBuffer = e.target.result;
                const hashBuffer = await crypto.subtle.digest('SHA-512', arrayBuffer);
                const hashArray = Array.from(new Uint8Array(hashBuffer));
                const hashHex = hashArray.map(b => b.toString(16).padStart(2, '0')).join('');
                resolve(hashHex);
            } catch (error) {
                reject(error);
            }
        };
        reader.onerror = () => reject(new Error('Failed to read file'));
        reader.readAsArrayBuffer(file);
    });
}

function enableButtons() {
    addHashBtn.disabled = false;
    checkHashBtn.disabled = false;
}

function disableButtons() {
    addHashBtn.disabled = true;
    checkHashBtn.disabled = true;
}

async function addHashToStore() {
    if (!currentHash) {
        addResult('error', 'No hash available', 'Please select a file first');
        return;
    }

    try {
        addHashBtn.disabled = true;
        addResult('info', 'Adding hash to store...', 'Please wait...');

        const response = await fetch(`${API_BASE_URL}/add`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({ hash: currentHash }),
        });

        const result = await response.json();

        if (response.ok && result.success) {
            addResult('success', 'Hash added successfully', result.message);
        } else {
            addResult('error', 'Failed to add hash', result.message);
        }
    } catch (error) {
        addResult('error', 'Network error', error.message);
    } finally {
        addHashBtn.disabled = false;
    }
}

async function checkHashExists() {
    if (!currentHash) {
        addResult('error', 'No hash available', 'Please select a file first');
        return;
    }

    await checkHash(currentHash, 'file');
}

async function checkManualHash() {
    const hash = manualHashInput.value.trim();
    
    if (!hash) {
        addResult('error', 'No hash provided', 'Please enter a hash to check');
        return;
    }

    if (hash.length !== 128) {
        addResult('error', 'Invalid hash length', 'Hash must be exactly 128 characters');
        return;
    }

    if (!/^[0-9a-fA-F]{128}$/.test(hash)) {
        addResult('error', 'Invalid hash format', 'Hash must be in hexadecimal format');
        return;
    }

    await checkHash(hash, 'manual');
}

async function checkHash(hash, source) {
    try {
        const button = source === 'file' ? checkHashBtn : checkManualHashBtn;
        button.disabled = true;
        
        addResult('info', 'Checking hash...', 'Please wait...');

        const response = await fetch(`${API_BASE_URL}/check`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({ hash: hash }),
        });

        const result = await response.json();

        if (response.ok && result.success) {
            const status = result.exists ? 'found' : 'not found';
            const message = result.exists 
                ? 'Hash exists in the store' 
                : 'Hash does not exist in the store';
            
            addResult(
                result.exists ? 'success' : 'info',
                `Hash ${status}`,
                message
            );
        } else {
            addResult('error', 'Failed to check hash', result.message);
        }
    } catch (error) {
        addResult('error', 'Network error', error.message);
    } finally {
        const button = source === 'file' ? checkHashBtn : checkManualHashBtn;
        button.disabled = false;
    }
}

function addResult(type, title, message) {
    const resultItem = document.createElement('div');
    resultItem.className = `result-item ${type}`;
    
    const timestamp = new Date().toLocaleTimeString();
    
    resultItem.innerHTML = `
        <h3>${title}</h3>
        <p>${message}</p>
        <p class="timestamp">${timestamp}</p>
    `;
    
    results.insertBefore(resultItem, results.firstChild);
    
    // Keep only the last 10 results
    const resultItems = results.querySelectorAll('.result-item');
    if (resultItems.length > 10) {
        results.removeChild(resultItems[resultItems.length - 1]);
    }
}

// Handle Enter key in manual hash input
manualHashInput.addEventListener('keypress', (e) => {
    if (e.key === 'Enter') {
        checkManualHash();
    }
});

// Initial setup
addResult('info', 'Frontend ready', 'Upload a file or enter a hash to get started'); 