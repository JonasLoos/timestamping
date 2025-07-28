// API base URL
const API_BASE_URL = 'http://127.0.0.1:3000';

// DOM elements
const fileInput = document.getElementById('fileInput');
const filesInfo = document.getElementById('filesInfo');
const filesList = document.getElementById('filesList');
const addAllHashesBtn = document.getElementById('addAllHashesBtn');
const checkAllHashesBtn = document.getElementById('checkAllHashesBtn');
const results = document.getElementById('results');
const manualHashInput = document.getElementById('manualHashInput');
const checkManualHashBtn = document.getElementById('checkManualHashBtn');
const refreshStatsBtn = document.getElementById('refreshStatsBtn');
const totalHashes = document.getElementById('totalHashes');
const occupiedSlots = document.getElementById('occupiedSlots');
const totalSlots = document.getElementById('totalSlots');
const loadFactor = document.getElementById('loadFactor');

// Current files data
let currentFiles = [];
let fileHashes = new Map(); // Map of file name to hash

// Event listeners
fileInput.addEventListener('change', handleFilesSelect);
addAllHashesBtn.addEventListener('click', addAllHashesToStore);
checkAllHashesBtn.addEventListener('click', checkAllHashes);
checkManualHashBtn.addEventListener('click', checkManualHash);
refreshStatsBtn.addEventListener('click', refreshStats);

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
        handleFilesSelect();
    }
});

async function handleFilesSelect() {
    const files = Array.from(fileInput.files);
    if (files.length === 0) {
        hideFilesInfo();
        return;
    }

    currentFiles = files;
    fileHashes.clear();
    clearResults();
    
    showFilesInfo(files);
    
    // Calculate hashes for all files
    for (const file of files) {
        try {
            const hash = await calculateFileHash(file);
            fileHashes.set(file.name, hash);
            updateFileHashDisplay(file.name, hash);
        } catch (error) {
            addResult('error', `Error calculating hash for ${file.name}`, error.message);
        }
    }
    
    if (fileHashes.size > 0) {
        enableButtons();
    }
}

function showFilesInfo(files) {
    filesList.innerHTML = '';
    
    files.forEach(file => {
        const fileItem = document.createElement('div');
        fileItem.className = 'file-item';
        fileItem.innerHTML = `
            <div class="file-header">
                <span class="file-name">${file.name}</span>
                <span class="file-size">${formatFileSize(file.size)}</span>
            </div>
            <div class="file-hash" id="hash-${file.name}">
                <span class="hash-label">Hash (SHA-512):</span>
                <span class="hash-value">Calculating...</span>
            </div>
        `;
        filesList.appendChild(fileItem);
    });
    
    filesInfo.style.display = 'block';
}

function updateFileHashDisplay(fileName, hash) {
    const hashElement = document.getElementById(`hash-${fileName}`);
    if (hashElement) {
        const hashValue = hashElement.querySelector('.hash-value');
        hashValue.textContent = hash;
        hashValue.className = 'hash-value hash-display';
    }
}

function hideFilesInfo() {
    filesInfo.style.display = 'none';
    currentFiles = [];
    fileHashes.clear();
    disableButtons();
    clearResults();
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
    addAllHashesBtn.disabled = false;
    checkAllHashesBtn.disabled = false;
}

function disableButtons() {
    addAllHashesBtn.disabled = true;
    checkAllHashesBtn.disabled = true;
}

function clearResults() {
    results.innerHTML = '';
}

async function addAllHashesToStore() {
    if (fileHashes.size === 0) {
        addResult('error', 'No hashes available', 'Please select files first');
        return;
    }

    try {
        addAllHashesBtn.disabled = true;
        addResult('info', 'Adding hashes to store...', `Processing ${fileHashes.size} files...`);

        const hashes = Array.from(fileHashes.values());
        let successCount = 0;
        let errorCount = 0;

        for (const [fileName, hash] of fileHashes) {
            try {
                const response = await fetch(`${API_BASE_URL}/add`, {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/json',
                    },
                    body: JSON.stringify({ hash: hash }),
                });

                const result = await response.json();

                if (response.ok && result.success) {
                    successCount++;
                    addResult('success', `Hash added for ${fileName}`, 'Hash added successfully');
                } else {
                    errorCount++;
                    addResult('error', `Failed to add hash for ${fileName}`, result.message);
                }
            } catch (error) {
                errorCount++;
                addResult('error', `Network error for ${fileName}`, error.message);
            }
        }

        // Summary
        if (successCount > 0) {
            addResult('success', 'Upload Summary', 
                `Successfully added ${successCount} hashes${errorCount > 0 ? `, ${errorCount} failed` : ''}`);
            await refreshStats();
        }
    } catch (error) {
        addResult('error', 'Network error', error.message);
    } finally {
        addAllHashesBtn.disabled = false;
    }
}

async function checkAllHashes() {
    if (fileHashes.size === 0) {
        addResult('error', 'No hashes available', 'Please select files first');
        return;
    }

    try {
        checkAllHashesBtn.disabled = true;
        addResult('info', 'Checking hashes...', `Checking ${fileHashes.size} files...`);

        let foundCount = 0;
        let notFoundCount = 0;

        for (const [fileName, hash] of fileHashes) {
            try {
                const response = await fetch(`${API_BASE_URL}/check`, {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/json',
                    },
                    body: JSON.stringify({ hash: hash }),
                });

                const result = await response.json();

                if (response.ok && result.success) {
                    if (result.exists) {
                        foundCount++;
                        addResult('success', `Hash found for ${fileName}`, 'Hash exists in the store');
                    } else {
                        notFoundCount++;
                        addResult('info', `Hash not found for ${fileName}`, 'Hash does not exist in the store');
                    }
                } else {
                    addResult('error', `Failed to check hash for ${fileName}`, result.message);
                }
            } catch (error) {
                addResult('error', `Network error for ${fileName}`, error.message);
            }
        }

        // Summary
        if (foundCount > 0 || notFoundCount > 0) {
            addResult('info', 'Check Summary', 
                `Found ${foundCount} hashes, ${notFoundCount} not found`);
        }
    } catch (error) {
        addResult('error', 'Network error', error.message);
    } finally {
        checkAllHashesBtn.disabled = false;
    }
}

async function refreshStats() {
    try {
        refreshStatsBtn.disabled = true;
        
        const response = await fetch(`${API_BASE_URL}/stats`, {
            method: 'GET',
            headers: {
                'Content-Type': 'application/json',
            },
        });

        if (response.ok) {
            const stats = await response.json();
            
            totalHashes.textContent = stats.count.toLocaleString();
            occupiedSlots.textContent = stats.slots.toLocaleString();
            totalSlots.textContent = stats.total_slots.toLocaleString();
            
            // Calculate load factor as percentage
            const loadFactorPercent = ((stats.slots / stats.total_slots) * 100).toFixed(2);
            loadFactor.textContent = `${loadFactorPercent}%`;
        } else {
            addResult('error', 'Failed to fetch stats', 'Could not retrieve store statistics');
        }
    } catch (error) {
        addResult('error', 'Network error', error.message);
    } finally {
        refreshStatsBtn.disabled = false;
    }
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
        const button = source === 'manual' ? checkManualHashBtn : null;
        if (button) button.disabled = true;
        
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
        if (source === 'manual' && checkManualHashBtn) {
            checkManualHashBtn.disabled = false;
        }
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
    
    // Keep only the last 20 results for current uploads
    const resultItems = results.querySelectorAll('.result-item');
    if (resultItems.length > 20) {
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
refreshStats(); // Load initial stats 