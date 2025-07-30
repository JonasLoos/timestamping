// API base URL
const API_BASE_URL = 'http://127.0.0.1:3427';

// DOM elements
const fileInput = document.getElementById('fileInput');
const filesTabsSection = document.getElementById('filesTabsSection');
const tabsHeader = document.getElementById('tabsHeader');
const tabsContent = document.getElementById('tabsContent');
const addAllHashesBtn = document.getElementById('addAllHashesBtn');
const checkAllHashesBtn = document.getElementById('checkAllHashesBtn');
const manualHashInput = document.getElementById('manualHashInput');
const checkManualHashBtn = document.getElementById('checkManualHashBtn');
const manualCheckResult = document.getElementById('manualCheckResult');
const refreshStatsBtn = document.getElementById('refreshStatsBtn');
const updateTreeBtn = document.getElementById('updateTreeBtn');
const totalHashes = document.getElementById('totalHashes');
const occupiedSlots = document.getElementById('occupiedSlots');
const totalSlots = document.getElementById('totalSlots');
const loadFactor = document.getElementById('loadFactor');
const merkleTreeSize = document.getElementById('merkleTreeSize');
const merkleRoot = document.getElementById('merkleRoot');
const lastTreeUpdate = document.getElementById('lastTreeUpdate');

// Modal elements
const proofModal = document.getElementById('proofModal');
const closeModal = document.getElementById('closeModal');
const proofFileName = document.getElementById('proofFileName');
const proofFileHash = document.getElementById('proofFileHash');
const proofExpectedRoot = document.getElementById('proofExpectedRoot');
const verificationResult = document.getElementById('verificationResult');
const verificationStatus = document.getElementById('verificationStatus');
const verificationDetails = document.getElementById('verificationDetails');
const proofSteps = document.getElementById('proofSteps');

// Current files data
let currentFiles = [];
let fileHashes = new Map(); // Map of file name to hash
let fileStatuses = new Map(); // Map of file name to status info
let fileProofs = new Map(); // Map of file name to merkle proof
let activeTab = null;
let currentMerkleRoot = '';

// Event listeners
fileInput.addEventListener('change', handleFilesSelect);
addAllHashesBtn.addEventListener('click', addAllHashesToStore);
checkAllHashesBtn.addEventListener('click', checkAllHashes);
checkManualHashBtn.addEventListener('click', checkManualHash);
refreshStatsBtn.addEventListener('click', refreshStats);
updateTreeBtn.addEventListener('click', updateMerkleTree);

// Modal event listeners
closeModal.addEventListener('click', hideProofModal);
window.addEventListener('click', (e) => {
    if (e.target === proofModal) {
        hideProofModal();
    }
});

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
    fileStatuses.clear();
    fileProofs.clear();
    
    showFilesInfo(files);
    
    // Calculate hashes for all files
    for (const file of files) {
        updateFileStatus(file.name, 'upload', 'processing', 'Calculating hash...', '');
        try {
            const hash = await calculateFileHash(file);
            fileHashes.set(file.name, hash);
            updateFileHashDisplay(file.name, hash);
            updateFileStatus(file.name, 'upload', 'pending', 'Hash calculated', '');
            updateFileStatus(file.name, 'check', 'pending', 'Ready to check', '');
        } catch (error) {
            updateFileStatus(file.name, 'upload', 'error', `Error: ${error.message}`, '');
        }
    }
    
    if (fileHashes.size > 0) {
        enableButtons();
        // Auto-check all hashes after calculating them
        setTimeout(() => checkAllHashes(), 500);
    }
}

function showFilesInfo(files) {
    // Clear existing tabs
    tabsHeader.innerHTML = '';
    tabsContent.innerHTML = '';
    
    files.forEach((file, index) => {
        // Initialize status for this file
        fileStatuses.set(file.name, {
            upload: { status: 'pending', message: 'Not uploaded', timestamp: '' },
            check: { status: 'pending', message: 'Not checked', timestamp: '' }
        });
        
        // Create tab button
        const tabButton = document.createElement('button');
        tabButton.className = 'tab-button';
        tabButton.textContent = file.name;
        tabButton.dataset.filename = file.name;
        tabButton.addEventListener('click', () => switchTab(file.name));
        tabsHeader.appendChild(tabButton);
        
        // Create tab content
        const tabContentDiv = document.createElement('div');
        tabContentDiv.className = 'tab-content';
        tabContentDiv.id = `tab-${file.name}`;
        tabContentDiv.innerHTML = `
            <div class="file-info">
                <div class="file-header">
                    <span class="file-name">${file.name}</span>
                    <span class="file-size">${formatFileSize(file.size)}</span>
                </div>
                <div class="file-hash" id="hash-${file.name}">
                    <span class="hash-label">Hash (SHA-512):</span>
                    <span class="hash-value">Calculating...</span>
                </div>
            </div>
            <div class="file-status">
                <div class="status-item pending" id="upload-status-${file.name}">
                    <div class="status-label">Upload Status</div>
                    <div class="status-value">Not uploaded</div>
                    <div class="status-timestamp"></div>
                </div>
                <div class="status-item pending" id="check-status-${file.name}">
                    <div class="status-label">Check Status</div>
                    <div class="status-value">Not checked</div>
                    <div class="status-timestamp"></div>
                </div>
            </div>
        `;
        tabsContent.appendChild(tabContentDiv);
        
        // Activate first tab
        if (index === 0) {
            switchTab(file.name);
        }
    });
    
    filesTabsSection.style.display = 'block';
}

function switchTab(filename) {
    // Remove active class from all tabs and content
    document.querySelectorAll('.tab-button').forEach(btn => btn.classList.remove('active'));
    document.querySelectorAll('.tab-content').forEach(content => content.classList.remove('active'));
    
    // Add active class to selected tab and content
    const tabButton = document.querySelector(`[data-filename="${filename}"]`);
    const tabContent = document.getElementById(`tab-${filename}`);
    
    if (tabButton && tabContent) {
        tabButton.classList.add('active');
        tabContent.classList.add('active');
        activeTab = filename;
    }
}

function updateFileStatus(fileName, type, status, message, timestamp, merkleProof = null) {
    if (!timestamp) {
        timestamp = new Date().toLocaleTimeString();
    }
    
    // Update in memory
    if (!fileStatuses.has(fileName)) {
        fileStatuses.set(fileName, {
            upload: { status: 'pending', message: 'Not uploaded', timestamp: '' },
            check: { status: 'pending', message: 'Not checked', timestamp: '' }
        });
    }
    
    fileStatuses.get(fileName)[type] = { status, message, timestamp };
    
    // Store merkle proof if provided
    if (merkleProof) {
        fileProofs.set(fileName, merkleProof);
    }
    
    // Update DOM
    const statusElement = document.getElementById(`${type}-status-${fileName}`);
    if (statusElement) {
        statusElement.className = `status-item ${status}`;
        statusElement.querySelector('.status-value').textContent = message;
        statusElement.querySelector('.status-timestamp').textContent = timestamp;
        
        // Remove existing proof button
        const existingButton = statusElement.querySelector('.proof-button');
        if (existingButton) {
            existingButton.remove();
        }
        
        // Add proof button if merkle proof is available
        if (merkleProof && merkleProof.length > 0) {
            const proofButton = document.createElement('button');
            proofButton.className = 'proof-button';
            proofButton.textContent = 'View Proof';
            proofButton.onclick = () => showProofModal(fileName);
            statusElement.appendChild(proofButton);
        }
    }
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
    filesTabsSection.style.display = 'none';
    currentFiles = [];
    fileHashes.clear();
    fileStatuses.clear();
    fileProofs.clear();
    activeTab = null;
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
                // Convert to base64 instead of hex
                const hashBase64 = btoa(String.fromCharCode(...hashArray));
                resolve(hashBase64);
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

async function addAllHashesToStore() {
    if (fileHashes.size === 0) {
        return;
    }

    try {
        addAllHashesBtn.disabled = true;

        // Use batch endpoint for better performance
        const hashes = Array.from(fileHashes.values());
        
        // Update all files to processing status
        for (const fileName of fileHashes.keys()) {
            updateFileStatus(fileName, 'upload', 'processing', 'Adding to store...', '');
        }
        
        const response = await fetch(`${API_BASE_URL}/add-batch`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({ hashes: hashes }),
        });

        const result = await response.json();

        if (response.ok && result.success) {
            // Update individual file statuses based on batch results
            const fileNameArray = Array.from(fileHashes.keys());
            result.results.forEach((hashResult, index) => {
                const fileName = fileNameArray[index];
                if (hashResult.error) {
                    updateFileStatus(fileName, 'upload', 'error', `Failed: ${hashResult.error}`, '');
                } else if (hashResult.is_new) {
                    updateFileStatus(fileName, 'upload', 'success', 'Successfully added to store', '');
                } else {
                    updateFileStatus(fileName, 'upload', 'warning', 'Already exists in store', '');
                }
            });
            
            // Show batch summary
            console.log(`Batch processed: ${result.total_hashes} total, ${result.new_hashes} new, ${result.existing_hashes} existing`);
        } else {
            // Handle batch failure
            for (const fileName of fileHashes.keys()) {
                updateFileStatus(fileName, 'upload', 'error', `Batch failed: ${result.message}`, '');
            }
        }

        await refreshStats();
    } catch (error) {
        console.error('Batch request error:', error);
        // Handle network errors
        for (const fileName of fileHashes.keys()) {
            updateFileStatus(fileName, 'upload', 'error', `Network error: ${error.message}`, '');
        }
    } finally {
        addAllHashesBtn.disabled = false;
    }
}

async function checkAllHashes() {
    if (fileHashes.size === 0) {
        return;
    }

    try {
        checkAllHashesBtn.disabled = true;

        for (const [fileName, hash] of fileHashes) {
            updateFileStatus(fileName, 'check', 'processing', 'Checking in store...', '');
            
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
                        const proofInfo = result.merkle_proof ? 
                            ` (${result.merkle_proof.length} proof levels)` : 
                            ' (no proof available)';
                        updateFileStatus(fileName, 'check', 'success', `Found in store${proofInfo}`, '', result.merkle_proof);
                    } else {
                        updateFileStatus(fileName, 'check', 'warning', 'Not found in store', '');
                    }
                } else {
                    updateFileStatus(fileName, 'check', 'error', `Failed: ${result.message}`, '');
                }
            } catch (error) {
                updateFileStatus(fileName, 'check', 'error', `Network error: ${error.message}`, '');
            }
        }
    } finally {
        checkAllHashesBtn.disabled = false;
    }
}

async function updateMerkleTree() {
    try {
        updateTreeBtn.disabled = true;

        const response = await fetch(`${API_BASE_URL}/update-tree`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
        });

        const result = await response.json();

        if (response.ok && result.success) {
            await refreshStats();
        }
    } catch (error) {
        console.error('Failed to update Merkle tree:', error);
    } finally {
        updateTreeBtn.disabled = false;
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

            // Merkle tree stats
            merkleTreeSize.textContent = stats.merkle_tree_size.toLocaleString();
            
            if (stats.merkle_tree_root) {
                currentMerkleRoot = stats.merkle_tree_root;
                merkleRoot.textContent = `${stats.merkle_tree_root.substring(0, 16)}...${stats.merkle_tree_root.substring(stats.merkle_tree_root.length - 16)}`;
                merkleRoot.title = stats.merkle_tree_root; // Full hash on hover
            } else {
                currentMerkleRoot = '';
                merkleRoot.textContent = 'Not generated';
                merkleRoot.title = '';
            }

            if (stats.last_tree_update) {
                const updateTime = new Date(stats.last_tree_update * 1000);
                lastTreeUpdate.textContent = updateTime.toLocaleString();
            } else {
                lastTreeUpdate.textContent = 'Never';
            }
        }
    } catch (error) {
        console.error('Failed to refresh stats:', error);
    } finally {
        refreshStatsBtn.disabled = false;
    }
}

async function checkManualHash() {
    const hash = manualHashInput.value.trim();
    
    if (!hash) {
        showManualResult('error', 'No hash provided', 'Please enter a hash to check');
        return;
    }

    // Validate base64 format
    try {
        atob(hash);
    } catch (error) {
        showManualResult('error', 'Invalid hash format', 'Hash must be in base64 format');
        return;
    }

    // Validate length (64 bytes = 512 bits)
    const decodedBytes = new Uint8Array(atob(hash).split('').map(c => c.charCodeAt(0)));
    if (decodedBytes.length !== 64) {
        showManualResult('error', 'Invalid hash length', 'Hash must be exactly 64 bytes (512 bits)');
        return;
    }

    await checkHash(hash);
}

async function checkHash(hash) {
    try {
        checkManualHashBtn.disabled = true;
        showManualResult('info', 'Checking hash...', 'Please wait...');

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
                let message = 'Hash exists in the store';
                if (result.merkle_proof) {
                    message += ` with ${result.merkle_proof.length} proof levels available`;
                } else {
                    message += ' (no merkle proof available - tree not generated)';
                }
                showManualResult('success', 'Hash found', message, result.merkle_proof);
            } else {
                showManualResult('info', 'Hash not found', 'Hash does not exist in the store');
            }
        } else {
            showManualResult('error', 'Failed to check hash', result.message);
        }
    } catch (error) {
        showManualResult('error', 'Network error', error.message);
    } finally {
        checkManualHashBtn.disabled = false;
    }
}

function showManualResult(type, title, message, merkleProof = null) {
    let merkleProofHtml = '';
    if (merkleProof && merkleProof.length > 0) {
        merkleProofHtml = `
            <div class="merkle-proof">
                <h4>Merkle Proof:</h4>
                <div class="proof-levels">
                    ${merkleProof.map((level, index) => `
                        <div class="proof-level">
                            <strong>Level ${index + 1}:</strong>
                            <div class="proof-pair">
                                <div class="proof-hash">Left: ${level[0].substring(0, 16)}...${level[0].substring(level[0].length - 16)}</div>
                                <div class="proof-hash">Right: ${level[1].substring(0, 16)}...${level[1].substring(level[1].length - 16)}</div>
                            </div>
                        </div>
                    `).join('')}
                </div>
                <p class="proof-note">This proof can be used to verify the hash's inclusion in the merkle tree.</p>
            </div>
        `;
    }
    
    manualCheckResult.className = `manual-result ${type}`;
    manualCheckResult.innerHTML = `
        <h4>${title}</h4>
        <p>${message}</p>
        ${merkleProofHtml}
    `;
    manualCheckResult.style.display = 'block';
}

// Merkle Proof Modal Functions
function showProofModal(fileName) {
    const proof = fileProofs.get(fileName);
    const hash = fileHashes.get(fileName);
    
    if (!proof || !hash) {
        return;
    }
    
    // Set basic info
    proofFileName.textContent = fileName;
    proofFileHash.textContent = hash;
    proofExpectedRoot.textContent = currentMerkleRoot || 'Not available';
    
    // Show modal
    proofModal.style.display = 'block';
    
    // Verify proof
    verifyMerkleProof(hash, proof, currentMerkleRoot);
}

function hideProofModal() {
    proofModal.style.display = 'none';
}

async function verifyMerkleProof(leafHash, proof, expectedRoot) {
    verificationStatus.textContent = 'Verifying proof...';
    verificationResult.className = 'verification-result';
    verificationDetails.textContent = '';
    proofSteps.innerHTML = '';
    
    try {
        let currentHash = leafHash;
        const steps = [];
        
        // Add initial step
        steps.push({
            stepNumber: 0,
            operation: 'Starting with leaf hash',
            leftHash: '',
            rightHash: '',
            operator: '',
            result: currentHash,
            isCurrent: true
        });
        
        // Process each proof level
        for (let i = 0; i < proof.length; i++) {
            const [leftSibling, rightSibling] = proof[i];
            let leftHash, rightHash, operation;
            
            // Determine if current hash is left or right child
            if (leftSibling === currentHash) {
                // Current hash is the left child
                leftHash = currentHash;
                rightHash = rightSibling;
                operation = 'Concatenate as left child with right sibling';
            } else {
                // Current hash is the right child
                leftHash = leftSibling;
                rightHash = currentHash;
                operation = 'Concatenate as right child with left sibling';
            }
            
            // Convert base64 strings to byte arrays
            const leftBytes = new Uint8Array(atob(leftHash).split('').map(c => c.charCodeAt(0)));
            const rightBytes = new Uint8Array(atob(rightHash).split('').map(c => c.charCodeAt(0)));
            
            // Concatenate byte arrays (same as backend hasher.update() calls)
            const combined = new Uint8Array(leftBytes.length + rightBytes.length);
            combined.set(leftBytes);
            combined.set(rightBytes, leftBytes.length);
            
            // Hash the concatenated bytes
            const hashBuffer = await crypto.subtle.digest('SHA-512', combined);
            const hashArray = Array.from(new Uint8Array(hashBuffer));
            const newHash = btoa(String.fromCharCode(...hashArray));
            
            steps.push({
                stepNumber: i + 1,
                operation: operation,
                leftHash: leftHash,
                rightHash: rightHash,
                operator: '+',
                result: newHash,
                isCurrent: false
            });
            
            currentHash = newHash;
        }
        
        // Mark last step as current
        if (steps.length > 1) {
            steps[steps.length - 1].isCurrent = true;
            steps[0].isCurrent = false;
        }
        
        // Display all steps
        displayProofSteps(steps);
        
        // Check if computed root matches expected root
        const isValid = currentHash === expectedRoot;
        
        if (isValid) {
            verificationResult.className = 'verification-result success';
            verificationStatus.textContent = '✓ Proof Verified Successfully';
            verificationDetails.textContent = 'The computed root hash matches the expected merkle root. This proves the file was included in the merkle tree at the time of generation.';
        } else {
            verificationResult.className = 'verification-result error';
            verificationStatus.textContent = '✗ Proof Verification Failed';
            verificationDetails.textContent = `The computed root hash (${currentHash.substring(0, 16)}...${currentHash.substring(currentHash.length - 16)}) does not match the expected root. This could indicate the proof is invalid or the merkle tree has been updated.`;
        }
        
    } catch (error) {
        verificationResult.className = 'verification-result error';
        verificationStatus.textContent = '✗ Verification Error';
        verificationDetails.textContent = `Failed to verify proof: ${error.message}`;
        console.error('Proof verification error:', error);
    }
}

function displayProofSteps(steps) {
    proofSteps.innerHTML = '';
    
    steps.forEach((step, index) => {
        const stepDiv = document.createElement('div');
        stepDiv.className = `proof-step ${step.isCurrent ? 'current' : ''}`;
        
        if (step.stepNumber === 0) {
            // Initial step
            stepDiv.innerHTML = `
                <div class="step-header">
                    <div class="step-number">${step.stepNumber}</div>
                    <span>${step.operation}</span>
                </div>
                <div class="step-result">${step.result}</div>
            `;
        } else {
            // Hash combination step
            stepDiv.innerHTML = `
                <div class="step-header">
                    <div class="step-number">${step.stepNumber}</div>
                    <span>${step.operation}</span>
                </div>
                <div class="step-operation">SHA-512(left + right)</div>
                <div class="step-hashes">
                    <div class="step-hash">${step.leftHash}</div>
                    <div class="step-operator">${step.operator}</div>
                    <div class="step-hash">${step.rightHash}</div>
                </div>
                <div class="step-result">${step.result}</div>
            `;
        }
        
        proofSteps.appendChild(stepDiv);
    });
}

// Handle Enter key in manual hash input
manualHashInput.addEventListener('keypress', (e) => {
    if (e.key === 'Enter') {
        checkManualHash();
    }
});

// Handle Escape key to close modal
document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && proofModal.style.display === 'block') {
        hideProofModal();
    }
});

// Initial setup
refreshStats(); // Load initial stats 